use super::{Kind, Result};
use anyhow::{Context, bail};
use windows_sys::Win32::{
    System::{
        Power::{GetPwrCapabilities, SYSTEM_POWER_CAPABILITIES, SetSuspendState},
        Shutdown::{EWX_POWEROFF, EWX_SHUTDOWN, ExitWindowsEx},
    },
    UI::WindowsAndMessaging::{HWND_BROADCAST, PostMessageW, SC_MONITORPOWER, WM_SYSCOMMAND},
};
mod privilege;

pub(super) fn modern_standby() -> Result<bool> {
    let mut capabilities = SYSTEM_POWER_CAPABILITIES::default();
    if !unsafe { GetPwrCapabilities(&mut capabilities) } {
        return Err(std::io::Error::last_os_error()).context("无法读取Windows待机能力");
    }
    Ok(capabilities.AoAc)
}
pub(super) fn request(kind: Kind) -> Result<()> {
    if kind == Kind::ScreenOff {
        if unsafe { PostMessageW(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER as usize, 2) } == 0
        {
            return Err(std::io::Error::last_os_error()).context("系统拒绝关闭显示器请求");
        }
        return Ok(());
    }
    let mut privilege = privilege::ShutdownPrivilege::enable()?;
    let accepted = unsafe {
        match kind {
            Kind::Sleep | Kind::Hibernate => SetSuspendState(kind == Kind::Hibernate, false, false),
            Kind::Shutdown => ExitWindowsEx(EWX_SHUTDOWN | EWX_POWEROFF, 0) != 0,
            Kind::ScreenOff => unreachable!(),
        }
    };
    let result = if accepted {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
            .with_context(|| format!("系统拒绝{}请求", kind.label()))
    };
    match (result, privilege.restore()) {
        (Ok(()), restored) => restored,
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(restore)) => bail!("{error:#}；恢复系统动作权限失败：{restore:#}"),
    }
}
