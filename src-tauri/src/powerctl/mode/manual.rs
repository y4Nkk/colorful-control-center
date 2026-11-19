use super::{Target, links, selection};
use crate::{
    display,
    hal::{
        Hal,
        oem_owner::{self, FnKeyIdentity, FnKeyProcess},
    },
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FnKeyConfirmation {
    identity: FnKeyIdentity,
    mode_revision: String,
}

pub(crate) enum Apply {
    Applied,
    ConfirmFnKey(FnKeyConfirmation),
}

pub(crate) fn ensure_available(hal: &Hal, display: &display::Controller) -> Result<()> {
    hal.ensure_mode_change_allowed()?;
    if display.trial_pending()? {
        bail!("请先保留或恢复显示试用，再执行模式GPU联动");
    }
    Ok(())
}

pub(crate) fn permit(hal: &Hal, display: &display::Controller) -> Result<()> {
    ensure_available(hal, display)?;
    oem_owner::ensure_owner_absent()
}

pub(crate) fn apply(
    context: links::Context<'_>,
    display: &display::Controller,
    expected: &Target,
    confirmed_fn_key: Option<FnKeyConfirmation>,
) -> Result<Apply> {
    let hal = context.hal;
    ensure_available(hal, display)?;
    if let Some(approved) = &confirmed_fn_key {
        ensure!(
            expected.revision.as_deref() == Some(approved.mode_revision.as_str()),
            "确认的模式目标已变化，请重新选择"
        );
    }
    if let Some(fn_key) = FnKeyProcess::blocking()? {
        {
            let _gate = hal.begin_write()?;
            ensure_available(hal, display)?;
            selection::checked_target(&context, expected)?;
        }
        let Some(approved) = confirmed_fn_key else {
            return Ok(Apply::ConfirmFnKey(FnKeyConfirmation {
                identity: fn_key.identity(),
                mode_revision: expected.revision.clone().context("模式目标缺少当前修订")?,
            }));
        };
        fn_key.close(approved.identity)?;
    }
    selection::apply_preview(context, expected, &mut || permit(hal, display))?;
    Ok(Apply::Applied)
}
