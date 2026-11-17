//! 自动档案策略；设备写入仍由 application 和 apply 唯一执行。
#[cfg(not(test))]
mod app;
mod controller;
mod events;
mod model;
mod runtime;
mod storage;
mod worker;

pub(crate) use controller::Controller;
pub(crate) use model::{Record, Request, View};

pub(crate) fn ensure_inactive(path: &std::path::Path) -> anyhow::Result<()> {
    if storage::read(path)?.is_some_and(|record| record.enabled) {
        anyhow::bail!("供电/唤醒自动档案仍已启用，请先关闭该策略再启用原厂自动模式");
    }
    Ok(())
}

#[cfg(test)]
mod reset_tests;
#[cfg(test)]
mod tests;
