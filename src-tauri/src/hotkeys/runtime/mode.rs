use super::{actions, cancelled, mpsc, osd, store};
use crate::hotkeys::model::Record;
use crate::{gpu_power, gpuctl, hal, keymap, powerctl};
use anyhow::{Result, bail};
use std::{
    cell::{Cell, RefCell},
    path::Path,
};
use tauri::{AppHandle, Manager};

pub(super) fn execute(
    app: &AppHandle,
    path: &Path,
    approved: &mut Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
) -> (Result<actions::Completion>, bool) {
    let stopped = Cell::new(false);
    let driver = approved.mode_driver.clone();
    let approved = RefCell::new(approved);
    let mut permit = || -> Result<()> {
        if stopped.get() || cancelled(stopping) {
            stopped.set(true);
            bail!("模式循环监听已停止");
        }
        if store::load(path)?.record != **approved.borrow() {
            bail!("模式循环许可已撤销");
        }
        if app.state::<keymap::Controller>().recording_active()? {
            bail!("按键录制中，模式循环已取消");
        }
        if app.state::<crate::display::Controller>().trial_pending()? {
            bail!("显示试用中，模式循环已取消");
        }
        let hal = app.state::<hal::Hal>();
        hal.ensure_mode_change_allowed()?;
        crate::fanctl::dust::ensure_manual_allowed(&hal)?;
        hal::oem_owner::ensure_owner_absent()
    };
    let hal = app.state::<hal::Hal>();
    let gpu = app.state::<gpuctl::GpuController>();
    let power = app.state::<gpu_power::Controller>();
    let result = powerctl::mode::cycle::execute(
        powerctl::mode::links::Context::native(&hal, &gpu, &power),
        identity,
        driver.as_deref(),
        &mut |driver| store::bind_mode_driver(path, &mut approved.borrow_mut(), driver),
        &mut permit,
    )
    .map(|mode| actions::Completion {
        message: Some(osd::Message::result(
            "电源模式",
            ["安静", "省电", "性能", "娱乐", "静音增强"][mode as usize],
        )),
        show_osd: true,
    });
    if cancelled(stopping) {
        stopped.set(true);
    }
    (result, stopped.get())
}
