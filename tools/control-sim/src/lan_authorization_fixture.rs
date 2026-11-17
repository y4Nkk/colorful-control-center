//! 生产许可协议与私有临时日志；不加载设备、提权或访问注册表。
use crate::platform::lan::{
    authorization::{self, Task},
    model::Request,
    worker::{Intent, Invocation, Operation},
};
use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use windows_sys::Win32::{Foundation::*, System::Threading::*};

pub fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("child") => child(&args[1..]),
        Some("parent") if args.len() == 3 => parent(Path::new(&args[1]), &args[2]),
        _ => bail!("隔离LAN进程参数无效"),
    }
}
struct FileTask {
    root: PathBuf,
    dirty: bool,
}
impl Task for FileTask {
    fn execute(&mut self) -> Result<()> {
        fs::write(self.root.join("executed"), b"only-after-commit")?;
        fs::write(self.root.join("value"), b"1")?;
        self.dirty = true;
        Ok(())
    }
    fn rollback(&mut self) -> Result<()> {
        fs::write(self.root.join("value"), b"0")?;
        fs::write(self.root.join("restored"), b"same-authorized-process")?;
        self.dirty = false;
        Ok(())
    }
    fn dirty(&self) -> bool {
        self.dirty
    }
}
fn alive(id: u32) -> bool {
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, id) };
    if handle.is_null() {
        return false;
    }
    let result = unsafe { WaitForSingleObject(handle, 0) } == WAIT_TIMEOUT;
    unsafe {
        CloseHandle(handle);
    }
    result
}
fn child(args: &[String]) -> Result<()> {
    let invocation = Invocation::from_args(args)?;
    let operation = invocation.operation()?;
    let root = operation.recovery_file.parent().context("隔离目录缺失")?;
    fs::write(root.join("child.pid"), std::process::id().to_string())?;
    let result = authorization::serve(&invocation, || {
        fs::write(root.join("task.prepared"), b"only-after-permission")?;
        Ok(Box::new(FileTask {
            root: root.into(),
            dirty: false,
        }))
    });
    if !root.join("executed").exists() || root.join("restored").exists() {
        let reason = if alive(invocation.authorization.parent_id) {
            "已取消或补偿"
        } else {
            "主进程已退出，未执行或已补偿"
        };
        fs::write(root.join("cancelled"), reason)?;
    }
    fs::write(root.join("child.exit"), b"done")?;
    result
}
fn wait(root: &Path, name: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !root.join(name).exists() {
        if Instant::now() >= deadline {
            bail!("隔离父进程等待{name}超时");
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    Ok(())
}
fn parent(root: &Path, scenario: &str) -> Result<()> {
    let operation = Operation {
        intent: Intent::Apply {
            request: Request {
                expected_revision: "isolated".into(),
                device_id: "isolated".into(),
                power_saving: Some(true),
                apply_wake_patch: false,
            },
        },
        recovery_file: root.join("lan-recovery.json"),
    };
    let early_exit = scenario == "exit-before-ready";
    let session = authorization::prepare_with(operation, move |arguments| {
        if early_exit {
            return Ok(7);
        }
        let mut child = Command::new(std::env::current_exe()?)
            .arg("child")
            .args(arguments.split(' '))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        Ok(child.wait()?.code().unwrap_or(255) as u32)
    });
    if early_exit {
        fs::write(
            root.join("refused"),
            format!("{:#}", session.err().context("未拒绝提前退出")?),
        )?;
        return Ok(());
    }
    let mut session = session?;
    fs::write(root.join("parent.ready"), b"ready-not-execute")?;
    if scenario == "parent-death-after-execute" {
        session.execute()?;
        fs::write(root.join("parent.applied"), b"applied-not-finalized")?;
    }
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
        "cancel" | "parent-death" | "parent-death-after-execute" => drop(session),
        _ => bail!("未知隔离LAN场景"),
    }
    fs::write(root.join("parent.exit"), b"done")?;
    Ok(())
}
