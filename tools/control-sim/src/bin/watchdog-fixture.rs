//! 仅用于进程测试：传感器为固定输入，恢复只写本次隔离目录，不链接 HAL。
#![windows_subsystem = "windows"]
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/watchdog/client.rs"]
mod client;
#[path = "../../../../src-tauri/src/watchdog/protocol.rs"]
mod protocol;

use anyhow::{Context, Result, bail};
use std::{
    cell::Cell,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new(args.get(1).context("缺少隔离目录")?);
    let scenario = args.get(2).context("缺少场景")?;
    if ![
        "disarm",
        "direct-disarm",
        "killed-parent",
        "timeout",
        "child-exit",
        "lost-output",
        "sensor-loss",
        "recovery-failure",
        "late-gpu-loss",
        "initial-invalid",
        "initial-hot",
        "ready-timeout",
        "full-speed-hot",
        "full-speed-killed-parent",
        "full-speed-pending",
        "full-speed-lost-mode",
        "full-speed-sensor-loss",
        "mode-mirror",
        "arm-timeout",
    ]
    .contains(&scenario.as_str())
    {
        bail!("未知隔离场景");
    }
    if args.iter().any(|a| a == "--fan-watchdog") {
        let mut expected = if scenario == "direct-disarm" {
            vec!["--fan-watchdog", "--direct-fan-watchdog"]
        } else {
            vec!["--fan-watchdog"]
        };
        if args.iter().any(|a| a == "--full-speed-fan-watchdog") {
            expected.push("--full-speed-fan-watchdog");
        }
        if args[3..] != expected {
            bail!("监护角色参数必须位于角色选项之前");
        }
        fs::write(root.join("child.pid"), std::process::id().to_string())?;
        let result = child(root, scenario);
        if let Err(error) = &result {
            protocol::report_failure(&format!("{error:#}"));
        }
        fs::write(root.join("child.exit"), format!("{result:?}"))?;
        result
    } else {
        parent(root, scenario)
    }
}

fn parent(root: &Path, scenario: &str) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg(root).arg(scenario);
    let policy = if scenario.starts_with("full-speed-") {
        protocol::GuardPolicy::FirmwareFullSpeed
    } else {
        protocol::GuardPolicy::Protected
    };
    let guard = client::Watchdog::spawn(command, scenario == "direct-disarm", policy);
    if matches!(
        scenario,
        "initial-invalid" | "initial-hot" | "ready-timeout"
    ) {
        let error = guard.err().context("异常初始状态不应握手成功")?;
        fs::write(root.join("parent.result"), format!("{error:#}"))?;
        return Ok(());
    }
    let mut guard = guard?;
    guard.arm()?;
    if scenario.starts_with("full-speed-") && scenario != "full-speed-pending" {
        fs::write(root.join("mode"), "1")?;
        guard.commit_full_speed()?;
    }
    fs::write(root.join("parent.ready"), "ARMED")?;
    if scenario == "full-speed-hot" {
        for _ in 0..35 {
            guard.pulse()?;
            std::thread::sleep(Duration::from_millis(100));
        }
        let rejected = guard.prepare(protocol::GuardPolicy::Protected).unwrap_err();
        fs::write(root.join("rejected"), format!("{rejected:#}"))?;
        guard.pulse()?;
        guard.disarm()?;
        fs::write(root.join("parent.result"), "DISARMED")?;
        return Ok(());
    }
    if matches!(scenario, "full-speed-lost-mode" | "full-speed-sensor-loss") {
        fs::write(root.join("trigger"), "1")?;
    }
    if matches!(scenario, "disarm" | "direct-disarm") {
        for _ in 0..4 {
            guard.pulse()?;
            std::thread::sleep(Duration::from_millis(100));
        }
        guard.disarm()?;
        fs::write(root.join("parent.result"), "DISARMED")?;
        return Ok(());
    }
    if scenario == "timeout" {
        std::thread::sleep(Duration::from_millis(3800));
    }
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Err(error) = guard.pulse() {
            fs::write(root.join("parent.result"), format!("{error:#}"))?;
            return Ok(());
        }
        if !matches!(scenario, "killed-parent" | "full-speed-killed-parent")
            && Instant::now() >= deadline
        {
            bail!("监护未按期报告退出");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn child(root: &Path, scenario: &str) -> Result<()> {
    if scenario == "arm-timeout" {
        return arm_timeout(root);
    }
    let samples = Cell::new(0);
    protocol::run(
        protocol::read_lines(std::io::stdin()),
        || {
            let index = samples.get();
            samples.set(index + 1);
            match (scenario, index) {
                ("initial-invalid", _) | ("sensor-loss", 1..) => None,
                ("initial-hot", _) => Some((99.0, None)),
                ("mode-mirror", _) => Some((
                    fs::read_to_string(root.join("temperature"))
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(50.0),
                    None,
                )),
                ("full-speed-sensor-loss", _) if root.join("trigger").exists() => None,
                (scenario, _) if scenario.starts_with("full-speed-") => Some((99.0, Some(98.0))),
                ("late-gpu-loss", index) => Some((50.0, (index == 1).then_some(45.0))),
                ("ready-timeout", 0) => {
                    std::thread::sleep(Duration::from_secs(6));
                    Some((50.0, None))
                }
                ("child-exit", 1..) => std::process::exit(23),
                ("lost-output", 1) => {
                    close_output();
                    hold_until_release(root);
                    Some((50.0, None))
                }
                ("recovery-failure", 1..) => Some((96.0, None)),
                _ => Some((50.0, None)),
            }
        },
        || {
            let mut log = OpenOptions::new()
                .create(true)
                .append(true)
                .open(root.join("restore.log"))?;
            writeln!(log, "restore")?;
            log.sync_all()?;
            if scenario == "mode-mirror" && root.join("hold-recovery").exists() {
                let deadline = Instant::now() + Duration::from_secs(6);
                while !root.join("release-recovery").exists() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            if scenario == "recovery-failure" {
                bail!("隔离恢复被拒绝");
            }
            Ok(())
        },
        if std::env::args().any(|arg| arg == "--full-speed-fan-watchdog") {
            protocol::GuardPolicy::FirmwareFullSpeed
        } else {
            protocol::GuardPolicy::Protected
        },
        || {
            fs::read_to_string(root.join("mode")).is_ok_and(|s| s == "1")
                && !(scenario == "full-speed-lost-mode" && root.join("trigger").exists())
        },
    )
}

fn close_output() {
    use std::os::windows::io::AsRawHandle;
    // 只关闭本隔离子进程继承的 stdout，模拟进程仍活着但回报管道已断。
    assert_ne!(
        unsafe { windows_sys::Win32::Foundation::CloseHandle(std::io::stdout().as_raw_handle()) },
        0
    );
}

fn hold_until_release(root: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !root.join("release-output").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn arm_timeout(root: &Path) -> Result<()> {
    let input = protocol::read_lines(std::io::stdin());
    let mut output = std::io::stdout();
    writeln!(output, "READY")?;
    output.flush()?;
    if input.recv_timeout(Duration::from_secs(3))?.as_str() != "ARM" {
        bail!("缺少ARM");
    }
    // 模拟已收到接管许可，但ARMED回报丢失；等父进程关闭管道才开始恢复。
    while input.recv_timeout(Duration::from_secs(5)).is_ok() {}
    fs::write(root.join("restore.log"), "restore\n")?;
    let deadline = Instant::now() + Duration::from_secs(6);
    while !root.join("release-recovery").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
