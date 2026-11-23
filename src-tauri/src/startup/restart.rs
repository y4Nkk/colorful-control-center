use super::instance::Activation;
use crate::{
    controller_lease::{self, ControllerLease},
    hal::authorization::{
        self, AdministrativeAccess,
        session::events::{Binding, Event},
    },
    shutdown,
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    io,
    os::windows::io::{AsRawHandle, OwnedHandle},
    process::Command,
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{INFINITE, WaitForMultipleObjects, WaitForSingleObject},
};

pub(super) const ARGUMENT: &str = "--restart";
const READY_TIMEOUT_MS: u32 = 10_000;
const PARENT_EXIT_TIMEOUT_MS: u32 = 20_000;
const LEASE_TIMEOUT: Duration = Duration::from_secs(30);
static REQUEST: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestartMode {
    Interface,
    Administrator,
}

impl RestartMode {
    fn arguments(self, binding: &Binding, user: String) -> [String; 5] {
        let mode = match self {
            Self::Interface => "interface",
            Self::Administrator => "administrator",
        };
        [
            ARGUMENT.into(),
            mode.into(),
            binding.parent_id.to_string(),
            binding.token.clone(),
            user,
        ]
    }

    fn launch(self, arguments: &[String]) -> Result<OwnedHandle> {
        match self {
            Self::Interface => Ok(Command::new(std::env::current_exe()?)
                .args(arguments)
                .spawn()
                .context("无法启动新的应用进程")?
                .into()),
            Self::Administrator => authorization::launch_elevated_window(&arguments.join(" ")),
        }
    }
}

struct HandoffEvents {
    ready: Event,
    commit: Event,
    cancel: Event,
}

impl HandoffEvents {
    fn bind(binding: &Binding, event: fn(&Binding, &str) -> Result<Event>) -> Result<Self> {
        Ok(Self {
            ready: event(binding, "restart-ready")?,
            commit: event(binding, "restart-commit")?,
            cancel: event(binding, "restart-cancel")?,
        })
    }

    fn await_decision(&self, parent: &Event) -> Result<bool> {
        // 新进程等待旧进程的决定，不用自己的超时撤销已发出的就绪信号。
        let handles = [self.cancel.raw(), self.commit.raw(), parent.raw()];
        match unsafe { WaitForMultipleObjects(3, handles.as_ptr(), 0, INFINITE) } {
            WAIT_OBJECT_0 => Ok(false),
            status if status == WAIT_OBJECT_0 + 1 => Ok(true),
            status if status == WAIT_OBJECT_0 + 2 => Ok(false),
            _ => Err(io::Error::last_os_error()).context("等待原应用的重启许可失败"),
        }
    }
}

pub(crate) fn request_restart(mode: RestartMode) -> Result<()> {
    let _request = REQUEST
        .try_lock()
        .map_err(|_| anyhow::anyhow!("应用正在重启"))?;
    ensure!(!shutdown::closing(), shutdown::CLOSING);
    if mode == RestartMode::Administrator {
        match authorization::administrative_access().context("无法读取当前账户的管理员权限")?
        {
            AdministrativeAccess::CanElevate => (),
            AdministrativeAccess::Elevated => bail!("当前应用已具有管理员权限"),
            AdministrativeAccess::Standard => bail!(authorization::SAME_USER_ADMIN_REQUIRED),
        }
    }
    let user = authorization::user()?;
    let binding = Binding::new()?;
    let events = HandoffEvents::bind(&binding, Event::create)?;
    let result = (|| {
        let child = mode.launch(&mode.arguments(&binding, user))?;
        let process = child.as_raw_handle();
        wait_ready(process, &events.ready, READY_TIMEOUT_MS)?;
        shutdown::request_restart(|| {
            wait_ready(process, &events.ready, 0)?;
            events.commit.signal().context("提交重启许可失败")
        })
    })();
    if let Err(error) = result {
        events
            .cancel
            .signal()
            .with_context(|| format!("{error:#}；通知新进程取消交接失败"))?;
        return Err(error);
    }
    Ok(())
}

fn wait_ready(process: HANDLE, ready: &Event, timeout: u32) -> Result<()> {
    let handles = [process, ready.raw()];
    match unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, timeout) } {
        WAIT_OBJECT_0 => bail!("新进程在确认重启交接前退出，请重试"),
        status if status == WAIT_OBJECT_0 + 1 => Ok(()),
        WAIT_TIMEOUT => bail!("等待新进程就绪超时，请重试"),
        _ => Err(io::Error::last_os_error()).context("无法确认新进程已就绪"),
    }
}

pub(super) struct Handoff {
    pub activation: Activation,
    pub mode: RestartMode,
    parent: Event,
}

impl Handoff {
    pub(super) fn take_control(&self) -> Result<ControllerLease> {
        match unsafe { WaitForSingleObject(self.parent.raw(), PARENT_EXIT_TIMEOUT_MS) } {
            WAIT_OBJECT_0 => wait_for_lease(controller_lease::NAME, LEASE_TIMEOUT),
            WAIT_TIMEOUT => bail!("原应用尚未退出，未接管硬件"),
            _ => Err(io::Error::last_os_error()).context("无法确认原应用退出"),
        }
    }
}

pub(super) fn receive(args: &[String]) -> Result<Option<Handoff>> {
    let [flag, mode, parent_id, token, expected_user] = args else {
        bail!("应用重启参数无效");
    };
    ensure!(flag == ARGUMENT, "应用重启参数无效");
    let mode = match mode.as_str() {
        "interface" => RestartMode::Interface,
        "administrator" => RestartMode::Administrator,
        _ => bail!("应用重启方式无效"),
    };
    let binding = Binding {
        parent_id: parent_id.parse()?,
        token: token.clone(),
    };
    binding.validate()?;
    let user = authorization::user()?;
    ensure!(user == *expected_user, "重启需要当前 Windows 账户授权");
    if mode == RestartMode::Administrator {
        ensure!(authorization::elevated()?, "重启进程未获得管理员权限");
    }
    let events = HandoffEvents::bind(&binding, Event::open)?;
    let parent = Event::parent(binding.parent_id).context("无法打开原应用进程")?;
    let activation = Activation::retain(&user)?;
    events.ready.signal()?;
    Ok(events.await_decision(&parent)?.then_some(Handoff {
        activation,
        mode,
        parent,
    }))
}

fn wait_for_lease(name: &str, timeout: Duration) -> Result<ControllerLease> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(lease) = ControllerLease::try_acquire(name)? {
            return Ok(lease);
        }
        ensure!(
            Instant::now() < deadline,
            "旧主程序或独立恢复任务仍在运行，未接管硬件"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
#[path = "restart_tests.rs"]
mod tests;
