use super::{PREFERENCE_MUTATION, Preferences, read_from, write_to};
use crate::{powerctl::mode::profile::ApprovalPreview, profiles::application::Permission};
use anyhow::{Context, Result, anyhow, bail};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub(super) fn permission(dir: PathBuf, expected: Preferences) -> Permission {
    let expected = Arc::new(Mutex::new(expected));
    let watched = expected.clone();
    let check_dir = dir.clone();
    Permission::with_mode_driver(
        move || {
            let snapshot = watched
                .lock()
                .map_err(|_| anyhow!("启动档案许可状态异常"))?
                .clone();
            check(&check_dir, &snapshot)
        },
        move |facts| {
            let mut expected = expected
                .lock()
                .map_err(|_| anyhow!("启动档案许可状态异常"))?;
            bind(&dir, &mut expected, facts)
        },
        crate::hal::authorization::require_elevated,
    )
}
fn check(dir: &Path, expected: &Preferences) -> Result<()> {
    if read_from(dir)? != *expected {
        bail!("启动档案选择或许可已变化，未继续执行");
    }
    Ok(())
}
fn bind(dir: &Path, expected: &mut Preferences, facts: &ApprovalPreview) -> Result<()> {
    // 与执行中的HAL→启动配置顺序一致，选择保存也先完成模式预检再取此锁。
    let _lock = PREFERENCE_MUTATION
        .lock()
        .map_err(|_| anyhow!("启动档案配置状态异常"))?;
    check(dir, expected)?;
    let mut next = expected.clone();
    let binding = next.profile.as_mut().context("启动档案已取消")?;
    facts.check(binding.mode.as_ref().context("启动档案没有完整模式授权")?)?;
    let identity = &facts
        .clocks
        .driver
        .as_ref()
        .context("启动档案尚未取得实际GPU驱动")?
        .identity;
    if binding.bind_driver(identity)? {
        write_to(dir, &next).context("记录启动档案GPU驱动失败，未执行相关目标")?;
        *expected = next;
    }
    Ok(())
}
