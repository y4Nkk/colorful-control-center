//! 风扇加速偏移的寂静档对照：固定负载下比较自动档与寂静档在偏移 0/X 时的占空比。
//! 默认只读；--execute 才发送运行时风扇档位与偏移命令，结束（含 Ctrl+C）后恢复原状，
//! 不写 AppSettings。命令到 EC 的落点见 docs/oem-dchu-contract.md §5。

use anyhow::{Context, Result, bail};
use ccc_core::fan::{oem_duty_byte, oem_tachometer_rpm};
use clap::Args;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler};

use crate::dchu::Dchu;

static STOP: AtomicBool = AtomicBool::new(false);

/// 本机在寂静档重载下会长期停在 98–99°C；探测会主动压低风扇，接近该区间前就中止。
const THERMAL_ABORT_C: u8 = 95;

/// DSDT `DCHU.SCMD` 0x79/0x01：档位枚举 → EC 命令 0xD7 写入的位图。
const MODE_BITS: [(u8, u8); 7] = [
    (0, 0x02),
    (1, 0x10),
    (2, 0x08),
    (5, 0x01),
    (6, 0x04),
    (7, 0x20),
    (8, 0x40),
];

fn bits_of(mode: u8) -> Option<u8> {
    MODE_BITS
        .iter()
        .find(|(known, _)| *known == mode)
        .map(|(_, bits)| *bits)
}

fn mode_of(bits: u8) -> Option<u8> {
    MODE_BITS
        .iter()
        .find(|(_, known)| *known == bits)
        .map(|(mode, _)| *mode)
}

fn mode_name(bits: u8) -> &'static str {
    match mode_of(bits) {
        Some(0) => "自动",
        Some(1) => "全速",
        Some(2) => "静音",
        Some(5) => "能效优先",
        Some(6) => "自定义",
        Some(7) => "档位 7",
        Some(8) => "寂静模式",
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
    /// 负载占空比（%）
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

#[derive(Clone, Copy)]
struct Sample {
    cpu_duty: f64,
    cpu_rpm: f64,
    cpu_temp: u8,
    gpu_duty: f64,
    gpu_rpm: f64,
    gpu_temp: u8,
}

struct Row {
    at: u64,
    phase: &'static str,
    /// 是否处于稳定后的取样窗口。
    sampled: bool,
    live: u8,
    sample: Sample,
}

#[derive(Clone, Copy, Debug)]
struct Summary {
    n: usize,
    duty: f64,
    rpm: f64,
    temp: f64,
    gpu_duty: f64,
    gpu_rpm: f64,
}

fn live_bits(dchu: &Dchu) -> Result<u8> {
    Ok(dchu.read_package(13)?[14])
}

fn decode_sample(packet: &[u8; 256]) -> Option<Sample> {
    if packet.iter().all(|&byte| byte == 0) || packet.iter().all(|&byte| byte == 0xFF) {
        return None;
    }
    let rpm = |at: usize| oem_tachometer_rpm(u16::from_be_bytes([packet[at], packet[at + 1]]));
    let duty = |at: usize| f64::from(packet[at]) * 100.0 / 255.0;
    Some(Sample {
        cpu_rpm: rpm(2),
        cpu_duty: duty(16),
        cpu_temp: packet[18],
        gpu_rpm: rpm(4),
        gpu_duty: duty(19),
        gpu_temp: packet[21],
    })
}

/// 与桌面端手动偏移的放行条件对齐；另有娱乐档运行时 Turbo 无法由保存值判断。
fn blockers(caps: &[u8; 256], page4: &[u8], power_mode: u8, live: u8) -> Vec<String> {
    let mut issues = Vec::new();
    if caps[..2] != [1, 0] {
        issues.push("BIOS 特性表不是 1.0 版，无法确认寂静档与偏移能力".into());
    }
    if caps[20] & 0x10 != 0 {
        issues.push("RPM 风扇表机型不支持加速偏移".into());
    }
    if caps[20] & 1 == 0 {
        issues.push("固件未声明寂静档".into());
    }
    if u16::from_le_bytes([caps[10], caps[11]]) & (1 << 10) != 0 {
        issues.push("固件声明禁用风扇加速偏移（WMI96 bit10）".into());
    }
    if power_mode == 2 && page4[8] != 0 {
        issues.push("性能档的 Turbo 风扇已开启，请先关闭".into());
    }
    if page4[80] != 0 {
        issues.push("固件正在除尘".into());
    }
    if mode_of(live).is_none() {
        issues.push(format!(
            "运行时档位寄存器 {live:#04x} 不是已知档位，无法保证恢复"
        ));
    }
    issues
}

pub fn run(args: &FanOffsetArgs) -> Result<()> {
    let dchu = Dchu::load().context("加载 InsydeDCHU 失败（OEM 控制中心或其驱动未安装）")?;
    let caps = dchu.bios_capabilities()?;
    let page4 = dchu.read_settings(4, 0, 256)?;
    let power_mode = dchu.read_settings(1, 1, 1)?[0];
    let live = live_bits(&dchu)?;
    let threads = args.threads.unwrap_or_else(|| {
        thread::available_parallelism()
            .map_or(2, |n| n.get())
            .max(2)
            / 2
    });
    let plan = plan(args.offset);
    println!(
        "== 风扇加速偏移对照（{}）==",
        if args.execute {
            "执行"
        } else {
            "只读预览"
        }
    );
    println!(
        "运行时档位寄存器 cmd13[14]={live:#04x}（{}）；保存档位 page4[5]={}，保存偏移 page4[7]={}%，寂静偏好 page4[9]={}，Turbo 偏好 page4[8]={}，电源模式 page1[1]={power_mode}",
        mode_name(live),
        page4[5],
        page4[7],
        page4[9],
        page4[8]
    );
    println!(
        "计划：负载 {} 线程 × {}%，每阶段稳定 {} s 后取样 {} s；阶段 {}",
        threads,
        args.load,
        args.settle,
        args.window,
        plan.iter()
            .map(|phase| format!("{}@{}%", phase.name, phase.offset))
            .collect::<Vec<_>>()
            .join(" → ")
    );
    let issues = blockers(&caps, &page4, power_mode, live);
    for issue in &issues {
        println!("阻断：{issue}");
    }
    if !args.execute {
        println!("只读预览：未发送任何写入。加 --execute 才会切换风扇档位与偏移，并在结束后恢复。");
        return Ok(());
    }
    if !issues.is_empty() {
        bail!("前置条件不满足，未发送任何写入");
    }
    install_ctrl_handler()?;
    let mut csv = Csv::create(&args.out)?;
    let outcome = experiment(&dchu, args, threads, &plan, &mut csv);
    let restored = restore(&dchu, live, page4[7]);
    match (outcome, restored) {
        (Ok(_), Ok(())) if STOP.load(Ordering::Relaxed) => {
            println!("已中止，未做判定；取样保留在 {}", args.out);
            Ok(())
        }
        (Ok(rows), Ok(())) => {
            report(&rows, &plan);
            println!("逐秒取样已写入 {}", args.out);
            Ok(())
        }
        (outcome, restored) => Err(outcome.err().or_else(|| restored.err()).unwrap()),
    }
}

/// 返回 1 表示已处理，进程不会被立即终止，主循环得以恢复原状。
fn install_ctrl_handler() -> Result<()> {
    unsafe extern "system" fn handler(kind: u32) -> i32 {
        if matches!(kind, CTRL_C_EVENT | CTRL_BREAK_EVENT) {
            STOP.store(true, Ordering::Relaxed);
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

fn experiment(
    dchu: &Dchu,
    args: &FanOffsetArgs,
    threads: usize,
    plan: &[Phase; 6],
    csv: &mut Csv,
) -> Result<Vec<Row>> {
    let mut rows = Vec::new();
    {
        let _load = Load::start(threads, args.load);
        let mut expected = 0;
        for phase in plan {
            if STOP.load(Ordering::Relaxed) {
                break;
            }
            if let Some(mode) = phase.mode {
                dchu.set_fan_runtime(1, u32::from(mode))?;
                expected = bits_of(mode).context("未知的档位")?;
            }
            dchu.set_fan_runtime(14, u32::from(oem_duty_byte(f64::from(phase.offset))))?;
            sample(dchu, phase, expected, args, csv, &mut rows)?;
        }
    }
    Ok(rows)
}

fn sample(
    dchu: &Dchu,
    phase: &Phase,
    expected: u8,
    args: &FanOffsetArgs,
    csv: &mut Csv,
    rows: &mut Vec<Row>,
) -> Result<()> {
    println!("阶段 {}@{}%…", phase.name, phase.offset);
    let started = Instant::now();
    let mut fresh = Vec::new();
    let mut failures = 0usize;
    let mut hot = 0usize;
    while !STOP.load(Ordering::Relaxed) && started.elapsed().as_secs() < args.settle + args.window {
        let tick = Instant::now();
        let at = started.elapsed().as_secs();
        let reading = dchu
            .read_package(12)
            .and_then(|packet| Ok((live_bits(dchu)?, decode_sample(&packet))));
        match reading {
            Ok((live, Some(sample))) => {
                if live != expected {
                    bail!(
                        "阶段 {} 的运行时档位为 {live:#04x}（应为 {expected:#04x}），有其他程序在改风扇状态，已中止",
                        phase.name
                    );
                }
                // 实验会主动把偏移降到 0；持续过热时不再继续，由调用方恢复原状。
                hot = if sample.cpu_temp >= THERMAL_ABORT_C {
                    hot + 1
                } else {
                    0
                };
                if hot >= 3 {
                    bail!("CPU 温度连续 3 秒不低于 {THERMAL_ABORT_C}°C，已中止");
                }
                fresh.push(Row {
                    at,
                    phase: phase.name,
                    sampled: at >= args.settle,
                    live,
                    sample,
                });
            }
            _ => failures += 1,
        }
        if let Some(rest) = Duration::from_secs(1).checked_sub(tick.elapsed()) {
            thread::sleep(rest);
        }
    }
    let settled = fresh.iter().filter(|row| row.sampled).count();
    csv.append(&fresh)?;
    rows.extend(fresh);
    println!("…取样 {settled} 条（读取失败 {failures} 次）");
    Ok(())
}

fn restore(dchu: &Dchu, live: u8, offset: u8) -> Result<()> {
    let sent = mode_of(live)
        .context("原档位未知")
        .and_then(|mode| dchu.set_fan_runtime(1, u32::from(mode)))
        .and_then(|()| dchu.set_fan_runtime(14, u32::from(oem_duty_byte(f64::from(offset)))));
    let now = live_bits(dchu);
    match (&sent, &now) {
        (Ok(()), Ok(bits)) if *bits == live => {
            println!(
                "已恢复原档位 {}（{live:#04x}）与保存偏移 {offset}%。",
                mode_name(live)
            );
            Ok(())
        }
        _ => bail!(
            "恢复未确认（发送 {sent:?}，档位回读 {now:?}）。请在控制台把风扇模式切回「{}」、加速偏移设为 {offset}%。",
            mode_name(live)
        ),
    }
}

struct Load {
    stop: Arc<AtomicBool>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl Load {
    fn start(count: usize, duty: u8) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let threads = (0..count)
            .map(|_| {
                let stop = stop.clone();
                thread::spawn(move || burn(&stop, duty))
            })
            .collect();
        Self { stop, threads }
    }
}

impl Drop for Load {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn burn(stop: &AtomicBool, duty: u8) {
    let period = Duration::from_millis(100);
    let busy = Duration::from_millis(u64::from(duty));
    while !stop.load(Ordering::Relaxed) {
        let started = Instant::now();
        while started.elapsed() < busy {
            std::hint::spin_loop();
        }
        if let Some(rest) = period.checked_sub(started.elapsed()) {
            thread::sleep(rest);
        }
    }
}

struct Csv(std::fs::File);

impl Csv {
    fn create(path: &str) -> Result<Self> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| format!("创建取样文件 {path} 失败（已存在时不会覆盖）"))?;
        writeln!(
            file,
            "t_s,phase,sampled,live_mode_bits,cpu_duty_pct,cpu_rpm,cpu_temp_c,gpu_duty_pct,gpu_rpm,gpu_temp_c"
        )?;
        Ok(Self(file))
    }

    fn append(&mut self, rows: &[Row]) -> Result<()> {
        for row in rows {
            let sample = &row.sample;
            writeln!(
                self.0,
                "{},{},{},{:#04x},{:.1},{:.0},{},{:.1},{:.0},{}",
                row.at,
                row.phase,
                u8::from(row.sampled),
                row.live,
                sample.cpu_duty,
                sample.cpu_rpm,
                sample.cpu_temp,
                sample.gpu_duty,
                sample.gpu_rpm,
                sample.gpu_temp
            )?;
        }
        self.0.flush()?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Response {
    Clear,
    None,
    Partial,
}

/// 判定阈值为经验值：占空比抬高 5 个百分点或温度压低 2°C 才算响应。
fn response(x: Summary, base: Summary) -> Response {
    let (duty, temp) = (x.duty - base.duty, x.temp - base.temp);
    if duty >= 5.0 || temp <= -2.0 {
        Response::Clear
    } else if duty <= 1.5 && temp >= -1.0 {
        Response::None
    } else {
        Response::Partial
    }
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    match values.len() % 2 {
        0 => (values[mid - 1] + values[mid]) / 2.0,
        _ => values[mid],
    }
}

fn summarize(rows: &[Row], phase: &str) -> Option<Summary> {
    let picked: Vec<Sample> = rows
        .iter()
        .filter(|row| row.phase == phase && row.sampled)
        .map(|row| row.sample)
        .collect();
    if picked.len() < 10 {
        return None;
    }
    let stat = |pick: fn(Sample) -> f64| {
        let mut values: Vec<f64> = picked.iter().map(|sample| pick(*sample)).collect();
        median(&mut values)
    };
    Some(Summary {
        n: picked.len(),
        duty: stat(|sample| sample.cpu_duty),
        rpm: stat(|sample| sample.cpu_rpm),
        temp: stat(|sample| f64::from(sample.cpu_temp)),
        gpu_duty: stat(|sample| sample.gpu_duty),
        gpu_rpm: stat(|sample| sample.gpu_rpm),
    })
}

/// 夹住对照阶段的前后两个基线取平均，抵消温度的单向漂移。
fn average(a: Summary, b: Summary) -> Summary {
    let mean = |x: f64, y: f64| (x + y) / 2.0;
    Summary {
        n: a.n + b.n,
        duty: mean(a.duty, b.duty),
        rpm: mean(a.rpm, b.rpm),
        temp: mean(a.temp, b.temp),
        gpu_duty: mean(a.gpu_duty, b.gpu_duty),
        gpu_rpm: mean(a.gpu_rpm, b.gpu_rpm),
    }
}

fn report(rows: &[Row], plan: &[Phase; 6]) {
    println!("\n阶段        样本  CPU占空比%  CPU转速  CPU温度°C  GPU占空比%  GPU转速");
    let stats =
        std::array::from_fn::<Option<Summary>, 6, _>(|index| summarize(rows, plan[index].name));
    for (phase, stat) in plan.iter().zip(&stats) {
        match stat {
            Some(stat) => println!(
                "{:<10} {:>4} {:>11.1} {:>8.0} {:>10.1} {:>11.1} {:>8.0}",
                phase.name, stat.n, stat.duty, stat.rpm, stat.temp, stat.gpu_duty, stat.gpu_rpm
            ),
            None => println!("{:<10} 数据不足", phase.name),
        }
    }
    let [auto0, auto_x, auto0b, quiet0, quiet_x, quiet0b] = stats;
    let (Some(auto0), Some(auto_x), Some(auto0b), Some(quiet0), Some(quiet_x), Some(quiet0b)) =
        (auto0, auto_x, auto0b, quiet0, quiet_x, quiet0b)
    else {
        println!("样本不足，未做判定。");
        return;
    };
    let (auto_base, quiet_base) = (average(auto0, auto0b), average(quiet0, quiet0b));
    for (label, base, x) in [
        ("自动档", auto_base, auto_x),
        ("寂静档", quiet_base, quiet_x),
    ] {
        println!(
            "{label}：偏移相对 0 → CPU 占空比 {:+.1} 个百分点，转速 {:+.0}，温度 {:+.1}°C；GPU 占空比 {:+.1}，转速 {:+.0}",
            x.duty - base.duty,
            x.rpm - base.rpm,
            x.temp - base.temp,
            x.gpu_duty - base.gpu_duty,
            x.gpu_rpm - base.gpu_rpm
        );
    }
    println!(
        "判定：{}",
        verdict(
            response(auto_x, auto_base),
            response(quiet_x, quiet_base),
            quiet_base.duty
        )
    );
    println!("结论仅适用于本次负载、电源模式与对照偏移。");
}

/// 本机寂静档在高温下两路占空比同为 0x8E（约 56%），疑似上限；基线贴近时偏移会被遮蔽。
const CAP_HINT: f64 = 54.0;

fn verdict(auto: Response, quiet: Response, quiet_base_duty: f64) -> &'static str {
    if auto != Response::Clear {
        return "无法判定：自动档对照组对偏移响应不足。请调整负载或对照偏移后重试。";
    }
    if quiet_base_duty >= CAP_HINT && quiet != Response::Clear {
        return "无法判定：寂静档基线占空比已接近高温下观察到的上限，偏移可能被上限遮蔽。请降低负载后重试。";
    }
    match quiet {
        Response::Clear => "寂静档生效，偏移在寂静档下同样有响应。",
        Response::None => "寂静档未生效，自动档有响应而寂静档的占空比与温度均未变化。",
        Response::Partial => "寂静档仅有部分响应，请结合上表与取样文件人工判断。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(duty: f64, temp: f64) -> Summary {
        Summary {
            n: 60,
            duty,
            rpm: duty * 80.0,
            temp,
            gpu_duty: duty,
            gpu_rpm: duty * 80.0,
        }
    }

    fn row(phase: &'static str, sampled: bool, duty: f64) -> Row {
        Row {
            at: 0,
            phase,
            sampled,
            live: 0x02,
            sample: Sample {
                cpu_duty: duty,
                cpu_rpm: duty * 80.0,
                cpu_temp: 70,
                gpu_duty: duty,
                gpu_rpm: 0.0,
                gpu_temp: 50,
            },
        }
    }

    #[test]
    fn mode_bits_round_trip_except_unassigned_modes() {
        assert_eq!(bits_of(0), Some(0x02));
        assert_eq!(bits_of(8), Some(0x40));
        assert_eq!(mode_of(0x40), Some(8));
        assert_eq!(
            (bits_of(3), bits_of(4), mode_of(0x00), mode_of(0x03)),
            (None, None, None, None)
        );
    }

    #[test]
    fn cmd12_decodes_like_the_desktop() {
        let mut packet = [0; 256];
        packet[2..8].copy_from_slice(&[0x01, 0xD6, 0x01, 0xD8, 0, 0]);
        packet[16..22].copy_from_slice(&[0x8E, 0x43, 0x5D, 0x8E, 0x4A, 0x50]);
        let sample = decode_sample(&packet).unwrap();
        assert_eq!(sample.cpu_rpm, oem_tachometer_rpm(0x01D6));
        assert_eq!(sample.gpu_rpm, oem_tachometer_rpm(0x01D8));
        assert!((sample.cpu_duty - f64::from(0x8E) * 100.0 / 255.0).abs() < 1e-9);
        assert_eq!((sample.cpu_temp, sample.gpu_temp), (0x5D, 0x50));
        assert!(decode_sample(&[0; 256]).is_none());
        assert!(decode_sample(&[0xFF; 256]).is_none());
    }

    #[test]
    fn blockers_mirror_manual_offset_gating() {
        let (mut caps, mut page4) = ([0; 256], [0; 256]);
        caps[0] = 1;
        caps[20] = 1;
        assert!(blockers(&caps, &page4, 3, 0x02).is_empty());
        assert!(blockers(&caps, &page4, 2, 0x40).is_empty());
        caps[20] = 0x11;
        assert!(
            blockers(&caps, &page4, 2, 0x02)
                .iter()
                .any(|issue| issue.contains("RPM"))
        );
        caps[20] = 0;
        assert!(
            blockers(&caps, &page4, 2, 0x02)
                .iter()
                .any(|issue| issue.contains("未声明寂静档"))
        );
        caps[20] = 1;
        caps[10..12].copy_from_slice(&[0x00, 0x04]);
        assert!(
            blockers(&caps, &page4, 2, 0x02)
                .iter()
                .any(|issue| issue.contains("bit10"))
        );
        caps[10..12].copy_from_slice(&[0, 0]);
        page4[8] = 1;
        assert!(
            blockers(&caps, &page4, 2, 0x02)
                .iter()
                .any(|issue| issue.contains("Turbo"))
        );
        assert!(blockers(&caps, &page4, 3, 0x02).is_empty());
        page4[8] = 0;
        page4[80] = 1;
        assert!(
            blockers(&caps, &page4, 2, 0x02)
                .iter()
                .any(|issue| issue.contains("除尘"))
        );
        page4[80] = 0;
        assert!(
            blockers(&caps, &page4, 2, 0x03)
                .iter()
                .any(|issue| issue.contains("恢复"))
        );
    }

    #[test]
    fn response_needs_a_clear_duty_or_temperature_change() {
        let base = summary(35.0, 75.0);
        assert_eq!(response(summary(50.0, 75.0), base), Response::Clear);
        assert_eq!(response(summary(35.0, 72.0), base), Response::Clear);
        assert_eq!(response(summary(35.5, 74.5), base), Response::None);
        assert_eq!(response(summary(37.5, 74.5), base), Response::Partial);
    }

    #[test]
    fn verdict_needs_a_control_and_refuses_a_capped_baseline() {
        assert!(verdict(Response::Clear, Response::Clear, 20.0).starts_with("寂静档生效"));
        assert!(verdict(Response::Clear, Response::None, 20.0).starts_with("寂静档未生效"));
        assert!(verdict(Response::None, Response::None, 20.0).starts_with("无法判定：自动档"));
        assert!(verdict(Response::Clear, Response::None, 55.7).starts_with("无法判定：寂静档基线"));
        assert!(verdict(Response::Clear, Response::Clear, 55.7).starts_with("寂静档生效"));
    }

    #[test]
    fn summarize_ignores_settling_samples_and_outliers() {
        let mut rows: Vec<Row> = (0..20).map(|_| row("auto-0", false, 99.0)).collect();
        rows.extend((0..9).map(|_| row("auto-0", true, 40.0)));
        rows.push(row("auto-0", true, 95.0));
        rows.push(row("quiet-0", true, 10.0));
        let stat = summarize(&rows, "auto-0").unwrap();
        assert_eq!(stat.n, 10);
        assert!((stat.duty - 40.0).abs() < 1e-9);
        assert!((stat.rpm - 3200.0).abs() < 1e-9);
        assert!(summarize(&rows, "auto-x").is_none());
    }
}
