//! 风扇加速偏移的寂静档对照。只操作运行时档位和偏移。
mod experiment;
mod load;
mod report;

use anyhow::{Context, Result, bail};
use ccc_core::{
    fan::{decode_oem_fan_mode, oem_duty_percent},
    fan_sensors::fan_channels,
};
use clap::Args;
use std::sync::atomic::AtomicBool;
use std::thread;
use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler};

use crate::dchu::Dchu;
use experiment::Baseline;

static STOP: AtomicBool = AtomicBool::new(false);

fn stopped() -> bool {
    STOP.load(std::sync::atomic::Ordering::Relaxed)
}

fn mode_name(mode: u8) -> &'static str {
    match mode {
        0 => "自动",
        1 => "全速",
        2 => "静音",
        5 => "能效优先",
        6 => "自定义",
        7 => "档位 7",
        8 => "寂静模式",
        _ => "未知档位",
    }
}

#[derive(Args)]
pub struct FanOffsetArgs {
    /// 实际发送运行时风扇档位与偏移并在结束后恢复；此操作会改变风扇转速
    #[arg(long)]
    execute: bool,
    /// 对照偏移（%）
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u8).range(10..=60))]
    offset: u8,
    /// 每阶段切换后的稳定时间（秒）
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(60..=900))]
    settle: u64,
    /// 每阶段的取样时间（秒）
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(30..=600))]
    window: u64,
    /// 负载线程数（默认为逻辑处理器数的一半）
    #[arg(long)]
    threads: Option<usize>,
    /// 负载线程的忙碌时间比例（%）
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u8).range(20..=100))]
    load: u8,
    /// 逐秒取样文件（CSV，不覆盖已有文件）
    #[arg(long, default_value = "fan-offset-ab.csv")]
    out: String,
}

struct Phase {
    name: &'static str,
    /// 仅在换档的阶段发送档位命令。
    mode: Option<u8>,
    offset: u8,
}

fn plan(offset: u8) -> [Phase; 6] {
    [
        Phase {
            name: "auto-0",
            mode: Some(0),
            offset: 0,
        },
        Phase {
            name: "auto-x",
            mode: None,
            offset,
        },
        Phase {
            name: "auto-0b",
            mode: None,
            offset: 0,
        },
        Phase {
            name: "quiet-0",
            mode: Some(8),
            offset: 0,
        },
        Phase {
            name: "quiet-x",
            mode: None,
            offset,
        },
        Phase {
            name: "quiet-0b",
            mode: None,
            offset: 0,
        },
    ]
}

fn blockers(caps: &[u8; 256], turbo_preferred: bool, power_mode: u8) -> Vec<&'static str> {
    let mut issues = Vec::new();
    if caps[..2] != [1, 0] {
        issues.push("BIOS 特性表不是 1.0 版，无法确认寂静档与偏移能力");
    }
    if caps[20] & 0x10 != 0 {
        issues.push("RPM 风扇表机型不支持加速偏移");
    }
    if caps[20] & 1 == 0 {
        issues.push("固件未声明寂静档");
    }
    if u16::from_le_bytes([caps[10], caps[11]]) & (1 << 10) != 0 {
        issues.push("固件声明禁用风扇加速偏移（WMI96 bit10）");
    }
    if power_mode == 2 && turbo_preferred {
        issues.push("性能档的 Turbo 风扇已开启，请先关闭");
    }
    issues
}

pub fn run(args: &FanOffsetArgs) -> Result<()> {
    let dchu = Dchu::load().context("加载 InsydeDCHU 失败（OEM 控制中心或其驱动未安装）")?;
    let threads = args.threads.unwrap_or_else(|| {
        thread::available_parallelism()
            .map_or(2, |n| n.get())
            .max(2)
            / 2
    });
    let phases = plan(args.offset);
    print_plan(args, threads, &phases);
    let Some(before) = preflight(&dchu, args.execute)? else {
        println!("只读预览：加 --execute 才会切换风扇档位与偏移，并在结束后恢复。");
        return Ok(());
    };
    if threads == 0 {
        bail!("负载线程数必须大于零");
    }
    install_ctrl_handler()?;
    let mut csv = report::Csv::create(&args.out)?;
    let result = experiment::run(
        &dchu,
        before,
        &phases,
        args,
        |row| csv.append(row),
        |sampled_at| load::Load::start(threads, args.load, sampled_at),
    )
    .with_context(|| format!("探测未发布判定。取样记录：{}", args.out))?;
    if let Some(rows) = result {
        report::report(&rows, &phases);
        println!("逐秒取样已写入 {}", args.out);
    } else {
        println!("已中止，取样记录保留在 {}", args.out);
    }
    Ok(())
}

fn print_plan(args: &FanOffsetArgs, threads: usize, phases: &[Phase; 6]) {
    println!(
        "== 风扇加速偏移对照（{}）==",
        if args.execute {
            "执行"
        } else {
            "只读预览"
        }
    );
    println!(
        "计划：负载 {} 线程 × {}%，每阶段稳定 {} s 后取样 {} s；阶段 {}",
        threads,
        args.load,
        args.settle,
        args.window,
        phases
            .iter()
            .map(|phase| format!("{}@{}%", phase.name, phase.offset))
            .collect::<Vec<_>>()
            .join(" → ")
    );
}

fn preflight(dchu: &Dchu, execute: bool) -> Result<Option<Baseline>> {
    let caps = dchu.bios_capabilities()?;
    let page4 = dchu.read_settings(4, 0, 256)?;
    let power_mode = dchu.read_settings(1, 1, 1)?[0];
    let info = dchu.read_package(13)?;
    let issues = blockers(&caps, page4[8] != 0, power_mode);
    for issue in &issues {
        println!("阻断：{issue}");
    }
    let bits = info[14];
    let mode = decode_oem_fan_mode(bits).map_err(anyhow::Error::msg);
    println!(
        "即时档位 {bits:#04x}（{}），保存档位 {}，保存偏移 {}%，电源模式 {power_mode}",
        mode.as_ref().map_or("未知档位", |&mode| mode_name(mode)),
        page4[5],
        page4[7],
    );
    let offset = dchu.fan_offset_raw();
    match &offset {
        Ok(raw) => println!("即时偏移 {}%（EC {raw:#04x}）", oem_duty_percent(*raw)),
        Err(error) => println!("阻断：即时偏移读取失败，{error:#}"),
    }
    if !execute {
        return Ok(None);
    }
    if !issues.is_empty() {
        bail!("前置条件不满足");
    }
    let channels = fan_channels(&info)
        .filter(|mask| mask & 0b11 == 0b11)
        .context("此对照需要确认 CPU 和 GPU 1 风扇通道")?;
    Ok(Some(Baseline {
        mode: mode?,
        offset: offset?,
        channels,
    }))
}

fn install_ctrl_handler() -> Result<()> {
    unsafe extern "system" fn handler(kind: u32) -> i32 {
        if matches!(kind, CTRL_C_EVENT | CTRL_BREAK_EVENT) {
            STOP.store(true, std::sync::atomic::Ordering::Relaxed);
            1
        } else {
            0
        }
    }
    if unsafe { SetConsoleCtrlHandler(Some(handler), 1) } == 0 {
        bail!("安装 Ctrl+C 处理失败");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_requires_declared_non_rpm_offset_and_rejects_performance_turbo() {
        let mut caps = [0; 256];
        caps[0] = 1;
        caps[20] = 1;
        assert!(blockers(&caps, false, 3).is_empty());
        assert!(
            blockers(&caps, true, 2)
                .iter()
                .any(|reason| reason.contains("Turbo"))
        );
        for byte in [0, 0x10, 0x11] {
            caps[20] = byte;
            assert!(!blockers(&caps, false, 3).is_empty());
        }
        caps[20] = 1;
        caps[11] = 1 << 2;
        assert!(
            blockers(&caps, false, 3)
                .iter()
                .any(|reason| reason.contains("bit10"))
        );
    }
}
