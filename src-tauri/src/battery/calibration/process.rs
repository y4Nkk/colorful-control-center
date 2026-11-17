use super::{
    native::Native,
    store::{FileStore, Store},
    worker,
};
use crate::{
    controller_lease::{ControllerLease, NAME},
    hal::Hal,
};
use anyhow::{Context, Result, bail};
use std::os::windows::ffi::OsStringExt;
use std::{io::Write, path::PathBuf, sync::mpsc, time::Duration};
use windows_sys::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_RoamingAppData, SHGetKnownFolderPath},
};

fn data_dir() -> Result<PathBuf> {
    let config: serde_json::Value = serde_json::from_str(include_str!("../../../tauri.conf.json"))?;
    let identifier = config["identifier"].as_str().context("应用标识缺失")?;
    let mut raw = std::ptr::null_mut();
    let code = unsafe {
        SHGetKnownFolderPath(&FOLDERID_RoamingAppData, 0, std::ptr::null_mut(), &mut raw)
    };
    if code < 0 || raw.is_null() {
        if !raw.is_null() {
            unsafe {
                CoTaskMemFree(raw.cast());
            }
        }
        bail!("应用数据目录不可用：HRESULT {code:#x}");
    }
    let end = (0..32_768).find(|index| unsafe { *raw.add(*index) } == 0);
    let base = end.map(|end| {
        PathBuf::from(std::ffi::OsString::from_wide(unsafe {
            std::slice::from_raw_parts(raw, end)
        }))
    });
    unsafe {
        CoTaskMemFree(raw.cast());
    }
    Ok(base.context("应用数据目录长度无效")?.join(identifier))
}
pub(crate) fn run_child() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args != ["--battery-calibration"] {
        bail!("校准工作入口不接受额外参数");
    }
    let _lease = ControllerLease::retain(NAME)?;
    let directory = data_dir()?;
    let store = FileStore(directory.join("battery-calibration.json"));
    let record = store.load()?.context("缺少预先准备的校准恢复记录")?;
    if !record.pending || record.state.is_some() || record.outcome.is_some() || record.armed {
        bail!("校准记录未准备，未初始化硬件通道");
    }
    let hal = Hal::new();
    let mut io = Native::task(
        &hal,
        directory.join("energy-policy.json"),
        &record.operation,
    )?;
    let input = worker::read(std::io::stdin());
    let (sender, output) = mpsc::sync_channel::<worker::Output>(16);
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stdout = std::io::stdout().lock();
        for message in output {
            let result = serde_json::to_writer(&mut stdout, &message)
                .map_err(std::io::Error::other)
                .and_then(|_| stdout.write_all(b"\n"))
                .and_then(|_| stdout.flush());
            if result.is_err() {
                break;
            }
        }
        let _ = done.send(());
    });
    let result = worker::run(input, &mut io, &store, |message| {
        sender
            .try_send(message)
            .map_err(|e| anyhow::anyhow!("校准输出管道：{e}"))
    });
    drop(sender);
    let _ = finished.recv_timeout(Duration::from_millis(500));
    result
}
