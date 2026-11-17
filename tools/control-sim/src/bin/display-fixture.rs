//! 只读写临时模式文件；复用实际显示监护协议、客户端及控制权对象。
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/controller_lease.rs"]
mod controller_lease;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/display/model.rs"]
mod display;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/display/guardian/mod.rs"]
mod guardian;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/storage.rs"]
mod storage;
use anyhow::{Context, Result, bail};
use display::{Access, Display, Mode};
use guardian::Monitor;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

struct FileDisplay(PathBuf);
impl Access for FileDisplay {
    fn list(&self) -> Result<Vec<Display>> {
        let mut displays = vec![];
        for file in ["mode.json", "mode2.json"] {
            match fs::read(self.0.join(file)) {
                Ok(bytes) => displays.push(Display {
                    current: serde_json::from_slice(&bytes)?,
                    frequencies: vec![60, 144],
                    primary: file == "mode.json",
                }),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(displays)
    }
    fn test(&self, _: &Mode) -> Result<()> {
        Ok(())
    }
    fn set(&self, mode: &Mode) -> Result<()> {
        let second = mode.device == "fixture-display-2";
        let file = mode_file(&self.0, mode)?;
        fs::write(
            self.0.join(if second {
                "restore2.attempt"
            } else {
                "restore.attempt"
            }),
            b"attempted",
        )?;
        if self.0.join("restore.blocked").exists()
            || second && self.0.join("restore2.blocked").exists()
        {
            bail!("模拟恢复暂不可用");
        }
        storage::write(&file, &serde_json::to_vec(mode)?)?;
        fs::write(
            self.0.join(if second { "restored2" } else { "restored" }),
            mode.frequency.to_string(),
        )?;
        Ok(())
    }
}
fn mode_file(root: &Path, mode: &Mode) -> Result<PathBuf> {
    Ok(root.join(match mode.device.as_str() {
        "fixture-display" => "mode.json",
        "fixture-display-2" => "mode2.json",
        _ => bail!("未知隔离显示器"),
    }))
}
fn initial() -> Mode {
    Mode {
        device: "fixture-display".into(),
        identity: "fixture-panel".into(),
        name: "Isolated".into(),
        width: 1920,
        height: 1080,
        bits: 32,
        orientation: 0,
        x: 0,
        y: 0,
        flags: 0,
        frequency: 60,
    }
}
fn child(root: &Path, name: &str, scenario: &str) -> Result<()> {
    let _lease = controller_lease::ControllerLease::retain(name)?;
    fs::write(root.join("child.pid"), std::process::id().to_string())?;
    if scenario == "never-ready" {
        std::thread::sleep(Duration::from_secs(5));
    }
    let input = guardian::protocol::read(std::io::stdin());
    guardian::protocol::run(input, &FileDisplay(root.to_owned()), |message| {
        if matches!(message, guardian::Output::Armed) {
            let _ = fs::write(root.join("guard.armed"), b"armed");
        }
        let mut output = std::io::stdout().lock();
        let _ = writeln!(output, "{}", serde_json::to_string(&message).unwrap());
        let _ = output.flush();
    })?;
    fs::write(root.join("child.exit"), b"finished")?;
    Ok(())
}
fn parent(root: &Path, name: &str, scenario: &str) -> Result<()> {
    let _lease =
        controller_lease::ControllerLease::try_acquire(name)?.context("隔离租约已被占用")?;
    let plan = prepare(root, scenario)?;
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("child").arg(root).arg(name).arg(scenario);
    let started = guardian::client::Client::spawn(command, plan.clone());
    if scenario == "never-ready" {
        assert!(started.is_err());
        fs::write(root.join("start.rejected"), b"rejected")?;
        return Ok(());
    }
    let guard = started?;
    assert!(root.join("guard.armed").exists());
    for (index, target) in plan.targets().iter().enumerate() {
        if scenario == "multi-partial" && index == 1 {
            continue;
        }
        storage::write(&mode_file(root, target)?, &serde_json::to_vec(target)?)?;
    }
    fs::write(root.join("parent.ready"), b"ready")?;
    finish_parent(
        root,
        scenario
            .strip_prefix("multi-")
            .or_else(|| scenario.strip_prefix("rotation-"))
            .unwrap_or(scenario),
        guard,
        &plan,
    )
}
fn prepare(root: &Path, scenario: &str) -> Result<guardian::Plan> {
    let mut modes = vec![initial()];
    if scenario.starts_with("rotation-") {
        modes[0].orientation = 1;
        modes[0].width = 1080;
        modes[0].height = 1920;
    }
    if scenario.starts_with("multi-") {
        let mut second = initial();
        second.device.push_str("-2");
        second.identity.push_str("-2");
        second.x = 1920;
        modes.push(second);
    }
    let mut changes = vec![];
    for before in modes {
        storage::write(&mode_file(root, &before)?, &serde_json::to_vec(&before)?)?;
        let target = if scenario.starts_with("rotation-") {
            before.oem_rotation()?
        } else {
            let mut target = before.clone();
            target.frequency = 144;
            target
        };
        changes.push(guardian::Change { before, target });
    }
    Ok(guardian::Plan {
        changes,
        timeout_ms: if scenario == "child-exit" { 3000 } else { 700 },
    })
}
fn finish_parent(
    root: &Path,
    scenario: &str,
    mut guard: guardian::client::Client,
    plan: &guardian::Plan,
) -> Result<()> {
    match scenario {
        "keep" => {
            guard.keep(&plan.targets())?;
            fs::write(root.join("parent.result"), b"kept")?;
        }
        "reject-keep" => {
            assert!(guard.keep(&plan.before()).is_err());
            fs::write(root.join("parent.result"), b"rejected")?;
        }
        "killed-parent" | "blocked-recovery" => loop {
            std::thread::sleep(Duration::from_millis(30));
        },
        "child-exit" => {
            wait(root, "kill.child")?;
            assert!(guard.recovered().is_err());
            for before in plan.before() {
                FileDisplay(root.to_owned()).set(&before)?;
            }
            guard.restored(&plan.before())?;
            fs::write(root.join("parent.result"), b"fallback")?;
        }
        _ => {
            let deadline = Instant::now() + Duration::from_secs(7);
            while !guard.recovered()? {
                if Instant::now() >= deadline {
                    bail!("隔离监护未恢复");
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            fs::write(root.join("parent.result"), b"recovered")?;
        }
    }
    Ok(())
}
fn wait(root: &Path, name: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(7);
    while !root.join(name).exists() {
        if Instant::now() >= deadline {
            bail!("等待隔离标记 {name} 超时");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new(args.get(2).context("缺少隔离目录")?);
    let name = args.get(3).context("缺少隔离租约")?;
    let scenario = args.get(4).context("缺少隔离场景")?;
    if args.get(1).map(String::as_str) == Some("child") {
        child(root, name, scenario)
    } else {
        parent(root, name, scenario)
    }
}
