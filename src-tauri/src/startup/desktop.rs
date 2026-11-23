use super::{instance::Activation, restart};
use crate::{
    controller_lease::{self, ControllerLease},
    hal::authorization,
};
use anyhow::{Context, Result, bail};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MessageBoxW,
};

pub(crate) const LOGIN_ARGUMENT: &str = "--autostart";

pub(crate) struct DesktopStart {
    pub activation: Activation,
    pub lease: ControllerLease,
    pub login: bool,
    pub reveal: bool,
}

pub(crate) fn prepare_desktop(args: &[String]) -> Result<Option<DesktopStart>> {
    let login = match args {
        [] => false,
        [flag] if flag == LOGIN_ARGUMENT => true,
        [flag, ..] if flag == restart::ARGUMENT => return resume_desktop(args),
        _ => bail!("应用启动参数无效"),
    };
    let user = authorization::user().context("无法确认当前 Windows 账户")?;
    let Some(activation) = Activation::claim(&user, !login)? else {
        return Ok(None);
    };
    let lease = match ControllerLease::try_acquire(controller_lease::NAME)? {
        Some(lease) => lease,
        None if login => return Ok(None),
        None => bail!("已有主程序或独立恢复任务正在控制硬件"),
    };
    Ok(Some(DesktopStart {
        activation,
        lease,
        login,
        reveal: false,
    }))
}

fn resume_desktop(args: &[String]) -> Result<Option<DesktopStart>> {
    let handoff = match restart::receive(args) {
        Ok(Some(handoff)) => handoff,
        Ok(None) => return Ok(None),
        Err(error) => {
            eprintln!("[restart] 交接前退出：{error:#}");
            std::process::exit(1);
        }
    };
    let lease = handoff.take_control().context("重启后无法接手控制台")?;
    Ok(Some(DesktopStart {
        activation: handoff.activation,
        lease,
        login: false,
        reveal: handoff.mode == restart::RestartMode::Administrator,
    }))
}

pub(crate) fn report_start_failure(error: &anyhow::Error) {
    let message = failure_message(error);
    eprintln!("[startup] {error:#}");
    let title: Vec<u16> = "Colorful Control Center 启动失败"
        .encode_utf16()
        .chain([0])
        .collect();
    let detail: Vec<u16> = message.encode_utf16().chain([0]).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            detail.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

fn failure_message(error: &anyhow::Error) -> String {
    format!("{error:#}\n\n请在上述问题解决后，从桌面或开始菜单重新打开应用。")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_failure_keeps_full_reason_and_retry_instruction() {
        let error = anyhow::anyhow!("独立恢复任务仍在运行").context("重启后无法接手控制台");
        assert!(failure_message(&error).contains("重启后无法接手控制台: 独立恢复任务仍在运行"));
        assert!(failure_message(&error).contains("重新打开应用"));
    }
}
