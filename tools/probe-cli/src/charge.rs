//! 充电保护阈值的真机验收。默认只读；`--execute` 才写入一组测试范围，并在结束或中断时恢复原值。
//! 数据包格式、候选校验和原厂通道直接编入桌面应用的源码，不另写一份。
// 内联模块里的 #[path] 从 src/charge/battery/ 起算，所以要多上两级到仓库根。
#[allow(dead_code)]
mod battery {
    #[path = "../../../../../src-tauri/src/battery/model.rs"]
    pub mod model;
    #[path = "../../../../../src-tauri/src/battery/protocol.rs"]
    pub mod protocol;
}

use crate::dchu::Dchu;
use anyhow::{Context, Result, bail};
use battery::model::{ChargeState, ChargeValues};
use clap::Args;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, SYSTEMTIME};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

#[derive(Args)]
pub struct ChargeArgs {
    /// 实际写入测试范围并恢复原值；此操作会临时改变固件的充电保护设置
    #[arg(long)]
    execute: bool,
    /// 测试范围的开始阈值（%），须在固件候选内
    #[arg(long, default_value_t = 60)]
    start: u8,
    /// 测试范围的停止阈值（%），须在固件候选内且高于开始阈值
    #[arg(long, default_value_t = 80)]
    stop: u8,
    /// 结果文件（JSON，不覆盖已有文件）
    #[arg(long, default_value = "charge-acceptance.json")]
    out: String,
}

#[derive(Serialize)]
struct Thresholds {
    enabled: bool,
    start: u8,
    stop: u8,
}

#[derive(Serialize)]
struct Step {
    name: &'static str,
    ok: bool,
    detail: String,
}

#[derive(Serialize)]
struct Report {
    generated_at: String,
    elevated: bool,
    mode: &'static str,
    start_candidates: Option<Vec<u8>>,
    stop_candidates: Option<Vec<u8>>,
    before: Option<Thresholds>,
    steps: Vec<Step>,
    passed: bool,
}

/// 写入测试范围后负责恢复；正常恢复完成后解除，中途出错或展开时仍会尽力写回原值。
struct Restore<'a> {
    dchu: &'a Dchu,
    original: ChargeState,
    armed: bool,
}

impl Drop for Restore<'_> {
    fn drop(&mut self) {
        if self.armed {
            match restore(self.dchu, &self.original) {
                Ok(()) => println!("出错后已恢复原值。"),
                Err(error) => println!(
                    "恢复原值失败：{error:#}。请在应用里重新设置一次「限制充电范围」。"
                ),
            }
        }
    }
}

fn thresholds(values: &ChargeValues) -> Thresholds {
    Thresholds {
        enabled: values.enabled,
        start: values.start,
        stop: values.stop,
    }
}

fn describe(values: &ChargeValues) -> String {
    let enabled = if values.enabled { "开启" } else { "关闭" };
    format!("{enabled}，开始 {}%，停止 {}%", values.start, values.stop)
}

/// 与桌面应用读取充电保护使用同一命令：cmd4 双向包，子命令 30。
fn read(dchu: &Dchu) -> Result<Option<ChargeState>> {
    let mut packet = [0; 256];
    packet[0] = 30;
    battery::protocol::charge(&dchu.exchange_package(4, &packet)?)
}

/// 与桌面应用写入充电保护使用同一命令：cmd4 双向包，子命令 31。
fn write(dchu: &Dchu, values: &ChargeValues) -> Result<()> {
    dchu.exchange_package(4, &battery::protocol::request(values))?;
    Ok(())
}

fn verify(dchu: &Dchu, expected: &ChargeState) -> Result<()> {
    let actual = read(dchu)?.context("固件未返回充电保护数据")?;
    if actual != *expected {
        bail!(
            "回读为{}，期望{}",
            describe(&actual.current),
            describe(&expected.current)
        );
    }
    Ok(())
}

/// 恢复原值并回读核对；固件没有一次接受时最多再试两次。
fn restore(dchu: &Dchu, original: &ChargeState) -> Result<()> {
    let mut failure = None;
    for attempt in 0..3 {
        if attempt > 0 {
            thread::sleep(Duration::from_millis(500));
        }
        let result = write(dchu, &original.current).and_then(|()| verify(dchu, original));
        match result {
            Ok(()) => return Ok(()),
            Err(error) => failure = Some(error),
        }
    }
    Err(failure.expect("至少尝试一次"))
}

fn battery_present() -> Result<bool> {
    let mut raw = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut raw) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // 官方定义：255 为未知，128 表示没有系统电池。
    Ok(raw.BatteryFlag != 255 && raw.BatteryFlag & 128 == 0)
}

fn elevated() -> bool {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION {
            TokenIsElevated: 0,
        };
        let mut length = 0u32;
        let read = GetTokenInformation(
            token,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut length,
        );
        CloseHandle(token);
        read != 0 && elevation.TokenIsElevated != 0
    }
}

fn local_time() -> String {
    let mut now = SYSTEMTIME::default();
    unsafe { GetLocalTime(&mut now) };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
    )
}

/// 写入与恢复之间只有毫秒级窗口；期间忽略 Ctrl+C，保证恢复能走完。
fn hold_interrupts() -> Result<()> {
    unsafe extern "system" fn hold(kind: u32) -> i32 {
        i32::from(matches!(kind, CTRL_C_EVENT | CTRL_BREAK_EVENT))
    }
    if unsafe { SetConsoleCtrlHandler(Some(hold), 1) } == 0 {
        bail!("安装 Ctrl+C 处理失败");
    }
    Ok(())
}

fn step<T>(report: &mut Report, name: &'static str, action: impl FnOnce() -> Result<T>) -> Result<T> {
    let result = action();
    let (ok, detail) = match &result {
        Ok(_) => (true, String::new()),
        Err(error) => (false, format!("{error:#}")),
    };
    report.steps.push(Step { name, ok, detail });
    result
}

fn accept(dchu: &Dchu, args: &ChargeArgs, report: &mut Report) -> Result<()> {
    let before = step(report, "读取原值", || {
        read(dchu)?.context("固件未返回充电保护数据")
    })?;
    report.start_candidates = Some(before.start_values.clone());
    report.stop_candidates = Some(before.stop_values.clone());
    report.before = Some(thresholds(&before.current));
    println!("当前充电保护：{}", describe(&before.current));
    println!(
        "可选阈值：开始 {}%，停止 {}%",
        before.start_values.iter().map(|value| value.to_string()).collect::<Vec<_>>().join("/"),
        before.stop_values.iter().map(|value| value.to_string()).collect::<Vec<_>>().join("/")
    );
    if !args.execute {
        println!("只读核对：加 --execute 才会写入测试范围，并在结束后恢复。");
        return Ok(());
    }
    step(report, "确认电池存在", || {
        if !battery_present().context("无法读取系统电源状态")? {
            bail!("未确认电池存在，未写入");
        }
        Ok(())
    })?;
    let target = step(report, "校验测试范围", || {
        before
            .target(&ChargeValues {
                enabled: true,
                start: args.start,
                stop: args.stop,
            })
            .context("测试范围不在固件候选内，或开始不低于停止")
    })?;
    hold_interrupts()?;
    let mut guard = Restore {
        dchu,
        original: before.clone(),
        armed: true,
    };
    step(report, "写入测试范围并回读", || {
        write(dchu, &target.current)?;
        verify(dchu, &target)
    })?;
    step(report, "恢复原值并回读", || restore(dchu, &before))?;
    guard.armed = false;
    thread::sleep(Duration::from_secs(2));
    step(report, "两秒后复核原值", || verify(dchu, &before))
}

pub fn run(args: &ChargeArgs) -> Result<()> {
    let mode = if args.execute {
        "执行"
    } else {
        "只读核对"
    };
    println!("== 充电保护阈值验收（{mode}）==");
    let dchu = Dchu::load().context("加载 InsydeDCHU 失败（OEM 控制中心或其驱动未安装）")?;
    let mut report = Report {
        generated_at: local_time(),
        elevated: elevated(),
        mode: if args.execute {
            "execute"
        } else {
            "read-only"
        },
        start_candidates: None,
        stop_candidates: None,
        before: None,
        steps: Vec::new(),
        passed: false,
    };
    let result = accept(&dchu, args, &mut report);
    report.passed = result.is_ok();
    for step in &report.steps {
        println!("  [{}] {}", if step.ok { "通过" } else { "失败" }, step.name);
        if !step.detail.is_empty() {
            println!("        {}", step.detail);
        }
    }
    println!(
        "权限：{}；结果：{}",
        if report.elevated { "管理员" } else { "普通用户" },
        if report.passed { "通过" } else { "未通过" }
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.out)
        .with_context(|| format!("创建结果文件 {} 失败（已存在时不会覆盖）", args.out))?;
    serde_json::to_writer_pretty(&mut file, &report).context("写入结果文件失败")?;
    writeln!(file).context("写入结果文件失败")?;
    println!("结果已写入 {}", args.out);
    result
}

#[cfg(test)]
mod tests {
    use super::battery::model::ChargeValues;
    use super::*;

    fn state() -> ChargeState {
        let mut packet = [0; 256];
        packet[..3].copy_from_slice(&[0, 95, 100]);
        packet[16..22].copy_from_slice(&[40, 50, 60, 70, 80, 95]);
        packet[32..37].copy_from_slice(&[60, 70, 80, 90, 100]);
        battery::protocol::charge(&packet).unwrap().unwrap()
    }

    #[test]
    fn test_range_must_come_from_firmware_candidates() {
        assert!(
            state()
                .target(&ChargeValues {
                    enabled: true,
                    start: 65,
                    stop: 80
                })
                .is_err()
        );
        assert!(
            state()
                .target(&ChargeValues {
                    enabled: true,
                    start: 70,
                    stop: 60
                })
                .is_err()
        );
        assert!(
            state()
                .target(&ChargeValues {
                    enabled: true,
                    start: 60,
                    stop: 80
                })
                .is_ok()
        );
    }

    #[test]
    fn failing_step_keeps_the_cause_in_the_report() {
        let mut report = Report {
            generated_at: String::new(),
            elevated: false,
            mode: "execute",
            start_candidates: None,
            stop_candidates: None,
            before: None,
            steps: Vec::new(),
            passed: false,
        };
        assert!(
            step(&mut report, "样例", || -> Result<()> { bail!("原因") }).is_err()
        );
        assert!(!report.steps[0].ok);
        assert!(report.steps[0].detail.contains("原因"));
    }
}
