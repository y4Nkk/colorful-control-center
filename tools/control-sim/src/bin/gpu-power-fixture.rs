//! 生产原厂GPU工作流的隔离进程夹具；不导入HAL、原厂DLL、PnP或UAC。
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/controller_lease.rs"]
mod controller_lease;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/gpu_power/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/gpu_power/operation.rs"]
mod operation;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/storage.rs"]
mod storage;
use anyhow::{Context, Result, bail};
use model::{Device, Plan, Target};
use operation::{Failure, Io, Phase};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
struct State {
    device: Device,
    firmware: bool,
}
#[derive(Serialize, Deserialize)]
struct Outcome {
    code: u32,
    message: String,
}
struct FileIo<'a> {
    root: &'a Path,
    scenario: &'a str,
}
fn save(root: &Path, name: &str, value: &impl Serialize) -> Result<()> {
    Ok(storage::write(
        &root.join(name),
        &serde_json::to_vec(value)?,
    )?)
}
fn state(root: &Path) -> Result<State> {
    Ok(serde_json::from_slice(&fs::read(root.join("state.json"))?)?)
}
impl FileIo<'_> {
    fn event(&self, event: String, mutate: impl FnOnce(&mut State)) -> Result<()> {
        let mut events: Vec<String> =
            serde_json::from_slice(&fs::read(self.root.join("events.json"))?)?;
        events.push(event);
        save(self.root, "events.json", &events)?;
        let number = events.len();
        let mut current = state(self.root)?;
        mutate(&mut current);
        save(self.root, "state.json", &current)?;
        if self.scenario == "hold" && number == 1 {
            fs::write(self.root.join("worker.paused"), b"paused")?;
            wait(self.root, "continue")?;
        }
        if (self.scenario == "compensate" && number == 4)
            || (self.scenario == "retry" && [5, 6].contains(&number))
        {
            return Err(operation::NativeCode(13).into());
        }
        Ok(())
    }
}
impl Io for FileIo<'_> {
    fn inspect(&self, id: &str) -> Result<Device> {
        let current = state(self.root)?.device;
        if current.id != id {
            bail!("隔离实例变化");
        }
        Ok(current)
    }
    fn firmware(&self, enabled: bool) -> Result<()> {
        self.event(format!("firmware:{enabled}"), |s| s.firmware = enabled)
    }
    fn node(&self, _: &str, enabled: bool) -> Result<()> {
        self.event(format!("node:{enabled}"), |s| {
            s.device.enabled = enabled;
            s.device.problem = if enabled { 0 } else { 22 };
        })
    }
    fn save(&self, automatic: bool) -> Result<()> {
        self.event(format!("automatic:{automatic}"), |s| {
            s.device.automatic_power_off = automatic
        })
    }
    fn pause(&self) {
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn wait(root: &Path, marker: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !root.join(marker).exists() {
        if Instant::now() >= deadline {
            bail!("等待隔离标记{marker}超时");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
fn child(root: &Path, lease: &str, scenario: &str, args: &[String]) -> Result<u32> {
    let _lease = controller_lease::ControllerLease::retain(lease)?;
    fs::write(root.join("child.pid"), std::process::id().to_string())?;
    fs::write(root.join("child.ready"), b"ready")?;
    let outcome = match args
        .first()
        .context("隔离计划缺失")
        .and_then(|encoded| serde_json::from_str::<Plan>(encoded).map_err(anyhow::Error::from))
        .map_err(|e| Failure::new(Phase::Request, e))
        .and_then(|plan| operation::execute(&FileIo { root, scenario }, &plan))
    {
        Ok(()) => Outcome {
            code: 0,
            message: "committed".into(),
        },
        Err(error) => Outcome {
            code: error.exit_code(),
            message: error.message,
        },
    };
    save(root, "child.result", &outcome)?;
    Ok(outcome.code)
}
fn run_worker(root: &Path, lease: &str, scenario: &str, plan: &Plan) -> Result<Outcome> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("worker")
        .arg(root)
        .arg(lease)
        .arg(scenario)
        .arg(serde_json::to_string(plan)?)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn()?;
    wait(root, "child.ready")?;
    fs::write(root.join("parent.ready"), b"ready")?;
    let status = child.wait()?;
    let outcome: Outcome = serde_json::from_slice(&fs::read(root.join("child.result"))?)?;
    if status.code().map(|code| code as u32) != Some(outcome.code) {
        bail!("隔离工作进程退出码与协议不一致");
    }
    Ok(outcome)
}
fn parent(root: &Path, lease: &str, scenario: &str) -> Result<()> {
    let _lease =
        controller_lease::ControllerLease::try_acquire(lease)?.context("隔离控制权已被占用")?;
    let initial = State {
        device: Device {
            id: "ACPI\\NVDA0820\\ISOLATED".into(),
            name: "Isolated".into(),
            enabled: true,
            automatic_power_off: false,
            problem: 0,
            drives_display: false,
            can_disable: true,
        },
        firmware: true,
    };
    save(root, "state.json", &initial)?;
    save(root, "events.json", &Vec::<String>::new())?;
    let plan = Plan {
        id: initial.device.id.clone(),
        expected: initial.device.target(),
        target: Target::requested(false),
    };
    let mut outcome = run_worker(root, lease, scenario, &plan)?;
    if scenario == "retry" {
        save(root, "first.result", &outcome)?;
        wait(root, "retry")?;
        let current = state(root)?;
        let recovery = Plan {
            id: current.device.id.clone(),
            expected: current.device.target(),
            target: initial.device.target(),
        };
        outcome = run_worker(root, lease, "success", &recovery)?;
    }
    save(root, "parent.result", &outcome)
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = Path::new(args.get(1).context("缺少隔离目录")?);
    let lease = args.get(2).context("缺少隔离租约")?;
    let scenario = args.get(3).context("缺少场景")?;
    if args.first().map(String::as_str) == Some("worker") {
        std::process::exit(child(root, lease, scenario, &args[4..])? as i32);
    }
    parent(root, lease, scenario)
}
