//! 登录启动唯一所有者：固定任务、当前用户、当前程序及首次前态。
mod model;
mod native;
mod transaction;
mod worker;
use anyhow::{Context, Result, bail};
pub use model::{Level, Snapshot};
use model::{Record, State};
use native::Native;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Mutex};
use tauri::Manager;
use transaction::Io;
pub(crate) use worker::run_child;
static MUTATION: Mutex<()> = Mutex::new(());
pub(super) const RECORD: &str = "login-startup.json";

fn path(app: &tauri::AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(RECORD))
}
fn record_path() -> Result<PathBuf> {
    let config: serde_json::Value = serde_json::from_str(include_str!("../../tauri.conf.json"))?;
    Ok(
        PathBuf::from(std::env::var_os("APPDATA").context("缺少用户目录")?)
            .join(config["identifier"].as_str().context("缺少应用标识")?)
            .join(RECORD),
    )
}
fn hash() -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(std::fs::read(std::env::current_exe()?)?)
    ))
}
fn inspect(native: &Native) -> Result<(Snapshot, State, Option<Record>)> {
    let state = native.state()?;
    let record = native.record()?;
    let mut level = model::validate(&state, &native.executable, &native.user)?;
    if state.task.is_none() && state.run.is_some() {
        level = Level::Standard;
    }
    Ok((
        Snapshot {
            revision: model::revision(&state, &record, &hash()?)?,
            level,
            elevated: crate::hal::authorization::elevated()?,
            can_restore: record.as_ref().is_some_and(|r| r.before != state),
            legacy: state.run.is_some(),
        },
        state,
        record,
    ))
}
pub fn read(app: &tauri::AppHandle) -> Result<Snapshot> {
    let _lock = MUTATION
        .lock()
        .map_err(|_| anyhow::anyhow!("登录自启状态异常"))?;
    Ok(inspect(&Native::new(path(app)?)?)?.0)
}
pub(crate) fn remove(expected_user: Option<&str>) -> Result<()> {
    let native = Native::new(record_path()?)?;
    if expected_user.is_some_and(|expected| expected != native.user) {
        bail!("卸载授权用户已变化，未清理其他用户的登录任务");
    }
    let expected = native.state()?;
    if expected.task.is_none() && expected.run.is_none() {
        return Ok(());
    }
    if !crate::hal::authorization::elevated()? {
        let code = crate::hal::authorization::launch(&format!("--remove-login {}", native.user))?;
        if code != 0 {
            bail!("登录任务清理未完成（{code}），保留安装文件");
        }
        return Ok(());
    }
    let record = native.record()?;
    transaction::apply(
        &native,
        &expected,
        &record,
        &State {
            task: None,
            run: None,
        },
    )?;
    native.save(&None)
}
pub fn apply(
    app: &tauri::AppHandle,
    expected_revision: &str,
    level: Level,
    restore: bool,
) -> Result<Snapshot> {
    let _lock = MUTATION
        .lock()
        .map_err(|_| anyhow::anyhow!("登录自启状态异常"))?;
    let native = Native::new(path(app)?)?;
    let (snapshot, expected, record) = inspect(&native)?;
    if snapshot.revision != expected_revision {
        bail!("登录自启预览已变化，请重新读取");
    }
    let target = if restore {
        record
            .as_ref()
            .context("没有登录自启恢复前态")?
            .before
            .clone()
    } else {
        State {
            task: model::xml(&native.executable, &native.user, level),
            run: None,
        }
    };
    let operation = worker::Operation {
        expected,
        record,
        target,
        executable_hash: hash()?,
        user: native.user.clone(),
        record_path: native.path.clone(),
    };
    // 所有任务配置统一在明确确认的系统会话中执行，撤销也不会悄悄降级。
    let mut session = worker::prepare(operation)?;
    let result = session.execute();
    let finish = session.finish();
    match (result, finish) {
        (Err(error), Err(cleanup)) => bail!("{error:#}；登录任务会话退出失败：{cleanup:#}"),
        (Err(error), _) | (_, Err(error)) => return Err(error),
        _ => (),
    }
    Ok(inspect(&native)?.0)
}

#[cfg(test)]
mod tests;
