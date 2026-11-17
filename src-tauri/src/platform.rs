//! 平台电源：PD等级事务与Deep S5只读状态。
use crate::hal::{Hal, oem_dchu::Dchu};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
#[path = "platform/scope.rs"]
mod scope;
#[path = "platform/target.rs"]
mod target;
pub(crate) use scope::Scope;
pub use target::{Encoding, Target};
pub(crate) use transaction::Plan;
#[path = "platform/lan.rs"]
pub mod lan;
#[path = "platform/pd_event.rs"]
pub(crate) mod pd_event;
#[path = "platform/protocol.rs"]
mod protocol;
#[cfg(test)]
#[path = "platform/tests.rs"]
mod tests;
#[path = "platform/transaction.rs"]
mod transaction;
#[path = "platform/usb.rs"]
pub mod usb;
use protocol::{Capabilities, Snapshot};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub level: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialLevel {
    pub value: u8,
    pub zero_based: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerDelivery {
    pub revision: String,
    pub encoding: Encoding,
    pub current_level: u8,
    pub current_valid: bool,
    pub available_levels: Vec<u8>,
    pub zero_based: bool,
    pub adapter_watts: Option<u8>,
    pub initial: Option<InitialLevel>,
    pub restore_blocked_reason: Option<String>,
    pub blocked_reason: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub power_delivery: Option<PowerDelivery>,
    pub deep_s5: Option<bool>,
    pub warnings: Vec<String>,
}
trait Io {
    fn capabilities(&self) -> Result<Capabilities>;
    fn pd_data(&self) -> Result<([u8; 2], [u8; 20])>;
    fn set_pd(&self, level: u8) -> Result<()>;
    fn deep_s5(&self) -> Result<bool>;
}

pub(crate) fn standby_display_policy(hal: &Hal) -> Result<crate::hal::power_events::DisplayPolicy> {
    let _gate = hal.begin_write()?;
    hal.with_dchu(|io| display_policy(io))
}
fn display_policy(io: &dyn Io) -> Result<crate::hal::power_events::DisplayPolicy> {
    use crate::hal::power_events::DisplayPolicy;
    if !io.capabilities()?.deep_s5 {
        return Ok(DisplayPolicy::Allowed);
    }
    Ok(if io.deep_s5()? {
        DisplayPolicy::DeepS5Enabled
    } else {
        DisplayPolicy::Allowed
    })
}
fn query(io: &Dchu, selector: u8) -> Result<[u8; 256]> {
    let mut packet = [0; 256];
    packet[0] = selector;
    io.exchange_package(4, &packet)
}
impl Io for Dchu {
    fn capabilities(&self) -> Result<Capabilities> {
        protocol::capabilities(&self.read_bios_capabilities(0, 25)?)
    }
    fn pd_data(&self) -> Result<([u8; 2], [u8; 20])> {
        let level = query(self, 54)?;
        let table = query(self, 61)?;
        Ok(([level[0], level[1]], table[..20].try_into()?))
    }
    fn set_pd(&self, level: u8) -> Result<()> {
        let mut packet = [0; 256];
        packet[0] = 55;
        packet[1] = level;
        self.exchange_package(4, &packet)?;
        Ok(())
    }
    fn deep_s5(&self) -> Result<bool> {
        match query(self, 57)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => bail!("Deep S5状态未知"),
        }
    }
}
fn capture(io: &impl Io) -> Result<Snapshot> {
    if !io.capabilities()?.pd {
        bail!("固件未声明PD档位控制");
    }
    let (level, table) = io.pd_data()?;
    Snapshot::decode(level, table)
}
fn read(
    io: &impl Io,
    baseline: &Option<Snapshot>,
    blocked_reason: Option<String>,
) -> Result<Profile> {
    let caps = io.capabilities()?;
    let mut warnings = Vec::new();
    let power_delivery = if caps.pd {
        capture(io)
            .map(|snapshot| PowerDelivery {
                revision: snapshot.revision(),
                encoding: snapshot.profile_target().encoding,
                current_level: snapshot.level(),
                current_valid: snapshot.validate_current().is_ok(),
                available_levels: snapshot.levels.clone(),
                zero_based: snapshot.zero_based(),
                adapter_watts: snapshot.adapter_watts(),
                initial: baseline.as_ref().map(|initial| InitialLevel {
                    value: initial.level(),
                    zero_based: initial.zero_based(),
                }),
                restore_blocked_reason: baseline
                    .as_ref()
                    .and_then(|initial| snapshot.restorable(initial).err())
                    .map(|e| format!("{e:#}")),
                blocked_reason: blocked_reason.or_else(|| {
                    baseline
                        .as_ref()
                        .and_then(|initial| snapshot.same_encoding(initial).err())
                        .map(|e| format!("{e:#}"))
                }),
            })
            .map_err(|e| warnings.push(format!("PD档位：{e:#}")))
            .ok()
    } else {
        if baseline.is_some() {
            warnings.push("PD初始快照仍保留，当前固件未声明控制能力".into());
        }
        None
    };
    let deep_s5 = if caps.deep_s5 {
        io.deep_s5()
            .map_err(|e| warnings.push(format!("Deep S5：{e:#}")))
            .ok()
    } else {
        None
    };
    Ok(Profile {
        power_delivery,
        deep_s5,
        warnings,
    })
}
#[derive(Default)]
pub struct Controller {
    baseline: Mutex<Option<Snapshot>>,
}
impl Controller {
    pub fn read_pd_event(&self, hal: &Hal) -> Result<pd_event::Profile> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let blocked = hal
            .ensure_mode_change_allowed()
            .err()
            .map(|error| format!("电池校准期间不能更改PD档位：{error:#}"));
        hal.with_dchu(|io| pd_event::preview(io, &scope.baseline, blocked))
    }
    pub fn run_pd_event(
        &self,
        hal: &Hal,
        expected: &str,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<bool> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.ensure_mode_change_allowed()
            .context("电池校准期间不能更改PD档位")?;
        hal.with_dchu(|io| pd_event::execute(io, &mut scope.baseline, expected, &mut permit))
    }
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let blocked = hal
            .ensure_mode_change_allowed()
            .err()
            .map(|e| format!("电池校准期间不能更改PD档位：{e:#}"));
        hal.with_dchu(|io| read(io, &scope.baseline, blocked))
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.ensure_mode_change_allowed()
            .context("电池校准期间不能更改PD档位")?;
        hal.with_dchu(|io| transaction::apply(io, &mut scope.baseline, request))
    }
    pub fn restore(&self, hal: &Hal, expected_revision: &str) -> Result<()> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.ensure_mode_change_allowed()
            .context("电池校准期间不能恢复PD档位")?;
        hal.with_dchu(|io| transaction::restore(io, &scope.baseline, expected_revision))
    }
}
