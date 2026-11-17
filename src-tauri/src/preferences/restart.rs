use crate::controller_lease::{self, ControllerLease};
use anyhow::{Context, Result, bail};
use std::{
    process::Command,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

const ARGUMENT: &str = "--restart-interface";

pub fn pending_lease() -> Result<Option<ControllerLease>> {
    if !std::env::args().any(|arg| arg == ARGUMENT) {
        return Ok(None);
    }
    wait_for_lease(controller_lease::NAME, Duration::from_secs(30)).map(Some)
}

pub(super) fn wait_for_lease(name: &str, timeout: Duration) -> Result<ControllerLease> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(lease) = ControllerLease::try_acquire(name)? {
            return Ok(lease);
        }
        if Instant::now() >= deadline {
            bail!("旧控制台或风扇监护尚未释放控制权；未接管硬件，请稍后重新打开应用");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn command(executable: &std::path::Path) -> Command {
    let mut command = Command::new(executable);
    command.arg(ARGUMENT);
    command
}

pub fn restart(app: &AppHandle) -> Result<()> {
    let state = app.state::<super::Controller>();
    if state.restarting.swap(true, Ordering::AcqRel) {
        bail!("应用正在重启");
    }
    let result = std::env::current_exe()
        .context("无法定位当前应用")
        .and_then(|executable| command(&executable).spawn().context("无法启动新的应用进程"));
    if let Err(error) = result {
        state.restarting.store(false, Ordering::Release);
        return Err(error);
    }
    app.exit(0);
    Ok(())
}
