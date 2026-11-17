//! 仅复用生产请求、许可和独显事务；设备是私有临时JSON，无原生HAL。
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/hal/authorization.rs"]
pub(crate) mod authorization;
mod hal {
    pub(crate) use crate::authorization;
}
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/controller_lease.rs"]
mod controller_lease;
#[path = "../../../../src-tauri/src/gpu_power/execution.rs"]
mod execution;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/gpu_power/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/gpu_power/operation.rs"]
mod operation;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/gpu_power/request.rs"]
mod request;
#[allow(dead_code)]
#[path = "../../../../src-tauri/src/gpu_power/sequence.rs"]
mod sequence;
#[path = "../../../../src-tauri/src/storage.rs"]
mod storage;

use anyhow::{Context, Result, bail};
use model::{Device, Plan, Target};
use operation::Io;
use request::{Invocation, Request};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
struct State {
    device: Device,
    firmware: bool,
    events: Vec<String>,
}
fn state(root: &Path) -> Result<State> {
    Ok(serde_json::from_slice(&fs::read(root.join("state.json"))?)?)
}
fn save(root: &Path, state: &State) -> Result<()> {
    Ok(storage::write(
        &root.join("state.json"),
        &serde_json::to_vec(state)?,
    )?)
}
fn wait(root: &Path, name: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !root.join(name).exists() {
        if Instant::now() >= deadline {
            bail!("隔离独显等待{name}超时");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
struct FileIo {
    root: PathBuf,
    hold: bool,
}
impl FileIo {
    fn event(&self, event: String, mutate: impl FnOnce(&mut State)) -> Result<()> {
        let mut current = state(&self.root)?;
        current.events.push(event);
        mutate(&mut current);
        save(&self.root, &current)?;
        if self.hold && !self.root.join("worker.paused").exists() {
            fs::write(self.root.join("worker.paused"), b"paused")?;
            wait(&self.root, "continue")?;
        }
        Ok(())
    }
}
impl Io for FileIo {
    fn inspect(&self, id: &str) -> Result<Device> {
        let device = state(&self.root)?.device;
        if device.id != id {
            bail!("隔离设备实例变化");
        }
        Ok(device)
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
}
fn child(root: &Path, lease: &str, scenario: &str, args: &[String]) -> Result<()> {
    let _lease = controller_lease::ControllerLease::retain(lease)?;
    let invocation = Invocation::from_args(args)?;
    fs::write(root.join("child.pid"), std::process::id().to_string())?;
    fs::write(
        root.join("session.path"),
        invocation.session_dir.to_string_lossy().as_bytes(),
    )?;
    let task_root = root.to_owned();
    let copied_scenario = scenario.to_owned();
    let mut stage = 0;
    let result = request::serve(&invocation, move |request| {
        stage += 1;
        fs::write(task_root.join("task.prepared"), b"only-after-permission")?;
        Ok(Box::new(execution::Execution::new(
            FileIo {
                root: task_root.clone(),
                hold: copied_scenario == "death-during-execute"
                    || copied_scenario == "sequence-death-third" && stage == 3,
            },
            request,
        )))
    });
    if let Err(error) = &result {
        authorization::session::report_failure(&invocation.session_dir, &format!("{error:#}"))?;
    }
    fs::write(root.join("child.exit"), format!("{result:?}"))?;
    result
}
fn authorize(
    root: &Path,
    lease: &str,
    scenario: &str,
    sequence: sequence::Sequence,
) -> Result<Box<dyn authorization::session::Authorized>> {
    let copied_root = root.to_owned();
    let copied_lease = lease.to_owned();
    let copied_scenario = scenario.to_owned();
    request::prepare_with(sequence, move |arguments| {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("child")
            .arg(copied_root)
            .arg(copied_lease)
            .arg(copied_scenario)
            .args(arguments.split(' '))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        Ok(command.spawn()?.wait()?.code().unwrap_or(255) as u32)
    })
}
fn planned_sequence(device: &Device, scenario: &str) -> Result<sequence::Sequence> {
    let targets = if scenario.starts_with("sequence-") {
        vec![
            Target::requested(false),
            Target {
                enabled: true,
                automatic_power_off: true,
            },
            Target {
                enabled: false,
                automatic_power_off: false,
            },
        ]
    } else {
        vec![Target::requested(false)]
    };
    let mut expected = device.clone();
    let mut requests = Vec::new();
    for target in targets {
        requests.push(Request {
            expected: expected.clone(),
            target,
            restore: false,
        });
        expected.enabled = target.enabled;
        expected.automatic_power_off = target.automatic_power_off;
        expected.problem = if target.enabled { 0 } else { 22 };
    }
    sequence::Sequence::new(&requests)
}
fn parent(root: &Path, lease: &str, scenario: &str) -> Result<()> {
    let _lease =
        controller_lease::ControllerLease::try_acquire(lease)?.context("隔离独显控制权被占用")?;
    let initial = State {
        device: Device {
            id: "ACPI\\NVDA0820\\ISOLATED".into(),
            name: "Private file".into(),
            enabled: true,
            automatic_power_off: false,
            problem: 0,
            drives_display: false,
            can_disable: true,
        },
        firmware: true,
        events: vec![],
    };
    save(root, &initial)?;
    let mut session = authorize(
        root,
        lease,
        scenario,
        planned_sequence(&initial.device, scenario)?,
    )?;
    if scenario.starts_with("sequence-") {
        session.execute()?;
        session.execute()?;
        if scenario == "sequence-partial-finish" {
            let error = session.finish().err().context("不完整序列未被拒绝")?;
            fs::write(root.join("parent.error"), format!("{error:#}"))?;
            fs::write(root.join("parent.exit"), b"done")?;
            return Ok(());
        }
        if scenario == "sequence-death-third" {
            session.execute()?;
        }
        fs::write(root.join("parent.ready"), b"ready")?;
        wait(root, "continue")?;
        drop(session);
        return Ok(());
    }
    if matches!(scenario, "death-applied" | "death-during-execute") {
        fs::write(root.join("parent.executing"), b"executing")?;
        session.execute()?;
    }
    fs::write(root.join("parent.ready"), b"ready")?;
    wait(root, "continue")?;
    match scenario {
        "commit" => {
            session.execute()?;
            session.finish()?;
        }
        "rollback" => {
            session.execute()?;
            session.rollback()?;
            session.finish()?;
        }
        "changed-request" => {
            let error = session.execute().err().context("变化请求未被拒绝")?;
            fs::write(root.join("parent.error"), format!("{error:#}"))?;
            session.finish()?;
        }
        "cancel" | "death-ready" | "death-applied" | "death-during-execute" => drop(session),
        _ => bail!("未知隔离独显场景"),
    }
    fs::write(root.join("parent.exit"), b"done")?;
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = Path::new(args.get(1).context("缺少隔离目录")?);
    let lease = args.get(2).context("缺少隔离控制权")?;
    let scenario = args.get(3).context("缺少隔离场景")?;
    match args.first().map(String::as_str) {
        Some("child") => child(root, lease, scenario, &args[4..]),
        Some("parent") => parent(root, lease, scenario),
        _ => bail!("隔离独显进程参数无效"),
    }
}
