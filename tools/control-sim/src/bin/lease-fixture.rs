//! 仅使用隔离名称和日志的控制权/监护组合，不链接 HAL 或启动应用。
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/watchdog/client.rs"]
mod client;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/controller_lease.rs"]
mod controller_lease;
#[path = "../../../../src-tauri/src/watchdog/protocol.rs"]
mod protocol;
use anyhow::{Context, Result, bail};
use controller_lease::ControllerLease;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

struct Config {
    root: PathBuf,
    scenario: String,
    name: String,
    label: String,
}

impl Config {
    fn write(&self, suffix: &str, value: impl AsRef<[u8]>) -> Result<()> {
        fs::write(self.root.join(format!("{}.{suffix}", self.label)), value)?;
        Ok(())
    }
    fn wait(&self, flag: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(6);
        while !self.root.join(flag).exists() {
            if Instant::now() >= deadline {
                bail!("等待隔离标记 {flag} 超时");
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let config = Config {
        root: PathBuf::from(args.get(1).context("缺少隔离目录")?),
        scenario: args.get(2).context("缺少场景")?.clone(),
        name: args.get(3).context("缺少隔离对象名")?.clone(),
        label: args.get(4).context("缺少日志标签")?.clone(),
    };
    if args.iter().any(|a| a == "--fan-watchdog") || config.scenario == "orphan" {
        config.write("child.pid", std::process::id().to_string())?;
        let result = guard(&config);
        config.write("child.exit", format!("{result:?}"))?;
        return result;
    }
    if !["owner", "probe", "watchdog"].contains(&config.scenario.as_str()) {
        bail!("未知隔离场景");
    }
    let lease = ControllerLease::try_acquire(&config.name)?;
    config.write("result", if lease.is_some() { "ACQUIRED" } else { "BUSY" })?;
    if lease.is_none() || config.scenario == "probe" {
        return Ok(());
    }
    let _lease = lease;
    if config.scenario == "watchdog" {
        parent(&config)?;
    } else {
        config.wait("release-parent")?;
    }
    Ok(())
}

fn parent(config: &Config) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg(&config.root)
        .arg(&config.scenario)
        .arg(&config.name)
        .arg(&config.label);
    let mut watchdog = client::Watchdog::spawn(command)?;
    watchdog.arm()?;
    config.write("ready", "ARMED")?;
    while !config.root.join("release-parent").exists() {
        if let Err(error) = watchdog.pulse() {
            config.write("watchdog-error", format!("{error:#}"))?;
            config.wait("release-parent")?;
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    watchdog.disarm()
}

fn guard(config: &Config) -> Result<()> {
    let _lease = ControllerLease::retain(&config.name)?;
    protocol::run(
        protocol::read_lines(std::io::stdin()),
        || Some((50.0, None)),
        || {
            config.write("recovery-started", "1")?;
            config.wait("release-recovery")?;
            config.write("restored", "1")
        },
    )
}
