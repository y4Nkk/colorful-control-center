//! 单独的假进程；只访问测试传入的私有目录，不初始化应用或硬件。
#[path = "../../../../src-tauri/src/owned_process.rs"]
mod owned_process;

use anyhow::{Context, Result, bail};
use owned_process::Process;
use std::{
    ffi::OsString,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

fn next(args: &mut impl Iterator<Item = OsString>) -> Result<OsString> {
    args.next().context("缺少隔离进程参数")
}
fn write(path: &Path, value: &str) -> Result<()> {
    let temporary = path.with_extension("pending");
    std::fs::write(&temporary, value)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}
fn wait(path: &Path) -> Result<()> {
    let end = Instant::now() + Duration::from_secs(30);
    while !path.exists() {
        if Instant::now() >= end {
            bail!("等待隔离控制文件超时：{}", path.display());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
fn child(mut args: impl Iterator<Item = OsString>) -> Result<()> {
    let ready = PathBuf::from(next(&mut args)?);
    let finish = PathBuf::from(next(&mut args)?);
    let values: Vec<String> = args
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    write(
        &ready,
        &serde_json::to_string(&(std::process::id(), values))?,
    )?;
    wait(&finish)
}
fn owner(directory: &Path) -> Result<()> {
    let executable = std::env::current_exe()?;
    let mut process = Process::prepare(
        &executable,
        &[
            "child".into(),
            directory.join("child-ready").into(),
            directory.join("finish-child").into(),
        ],
    )?;
    write(&directory.join("child-pid"), &process.id().to_string())?;
    wait(&directory.join("activate"))?;
    process.start()?;
    if process.poll()?.is_some() {
        bail!("隔离子进程启动后提前退出");
    }
    write(&directory.join("started"), "started")?;
    wait(&directory.join("stop"))?;
    process.stop()?;
    Ok(())
}
fn tree(directory: &Path) -> Result<()> {
    let executable = std::env::current_exe()?;
    let mut leaf = Command::new(executable)
        .creation_flags(CREATE_NO_WINDOW)
        .arg("child")
        .arg(directory.join("leaf-ready"))
        .arg(directory.join("finish-leaf"))
        .spawn()?;
    if let Err(error) = wait(&directory.join("leaf-ready")) {
        let _ = leaf.kill();
        let _ = leaf.wait();
        return Err(error);
    }
    Ok(())
}
fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    match next(&mut args)?.to_str() {
        Some("child") => child(args),
        Some("owner") => owner(Path::new(&next(&mut args)?)),
        Some("tree") => tree(Path::new(&next(&mut args)?)),
        _ => bail!("未知隔离进程模式"),
    }
}
