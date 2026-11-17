//! 原厂闲置关灯、开机覆盖与外围灯区；不拥有键盘模式或逐键矩阵。
use super::{protocol::*, *};
use anyhow::{Context, bail};
use ccc_core::settings::SettingsAccess;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[path = "options/cover.rs"]
mod cover;
#[path = "options/native.rs"]
pub(super) mod native;
#[path = "options/scope.rs"]
mod scope;
#[cfg(test)]
#[path = "options/tests.rs"]
mod tests;
#[path = "options/transaction.rs"]
mod transaction;
pub use self::scope::Target;
pub(crate) use self::scope::{Plan, Scope};
pub use cover::TopCover;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub keyboard_type: u8,
    pub peripheral_support_known: bool,
    pub boot_override: bool,
    pub logo: bool,
    pub light_bar: bool,
    pub top_cover: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdleTimer {
    pub enabled: bool,
    pub seconds: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Values {
    pub idle_timer: IdleTimer,
    pub boot_override: Option<bool>,
    pub logo: Option<Zone>,
    pub light_bar: Option<Zone>,
    pub top_cover: Option<TopCover>,
}
impl Values {
    pub fn validate(&self) -> Result<()> {
        if self.idle_timer.seconds > 7199 || self.idle_timer.enabled && self.idle_timer.seconds < 5
        {
            bail!("闲置关灯启用时须为5–7199秒");
        }
        if let Some(cover) = &self.top_cover {
            cover.validate()?;
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub values: Values,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub revision: String,
    pub current: Values,
    pub capabilities: Capabilities,
    pub can_restore: bool,
    pub initial: Option<Values>,
}
#[derive(Clone)]
struct Snapshot {
    caps: Capabilities,
    page: Vec<u8>,
}
impl Snapshot {
    fn revision(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(format!("{:?}:{:?}", self.caps, self.relevant()))
        )
    }
    fn relevant(&self) -> Vec<u8> {
        [&self.page[7..8], &self.page[35..40], &self.page[80..108]].concat()
    }
    fn values(&self) -> Result<Values> {
        let flag = |at| -> Result<bool> {
            match self.page[at] {
                0 => Ok(false),
                1 => Ok(true),
                _ => bail!("灯光保存开关 {at} 未知"),
            }
        };
        let zone = |at| -> Result<Zone> {
            Ok(Zone {
                enabled: flag(at)?,
                color: Rgb {
                    r: self.page[at + 1],
                    g: self.page[at + 2],
                    b: self.page[at + 3],
                },
            })
        };
        if self.page[37] > 1 || self.page[38] > 59 || self.page[39] > 59 {
            bail!("闲置关灯时间超出原厂范围，不能保证恢复");
        }
        let result = Values {
            idle_timer: IdleTimer {
                enabled: flag(36)?,
                seconds: u16::from(self.page[37]) * 3600
                    + u16::from(self.page[38]) * 60
                    + u16::from(self.page[39]),
            },
            boot_override: self.caps.boot_override.then(|| flag(7)).transpose()?,
            logo: self.caps.logo.then(|| zone(100)).transpose()?,
            light_bar: self.caps.light_bar.then(|| zone(104)).transpose()?,
            top_cover: self.caps.top_cover.then(|| TopCover {
                brightness: self.page[100],
                color: Rgb {
                    r: self.page[101],
                    g: self.page[102],
                    b: self.page[103],
                },
            }),
        };
        validate(&result, &self.caps)?;
        Ok(result)
    }
    fn target(&self, values: &Values) -> Result<Self> {
        validate(values, &self.caps)?;
        let mut result = self.clone();
        let timer = &values.idle_timer;
        result.page[36..40].copy_from_slice(&[
            u8::from(timer.enabled),
            (timer.seconds / 3600) as u8,
            (timer.seconds / 60 % 60) as u8,
            (timer.seconds % 60) as u8,
        ]);
        if let Some(value) = values.boot_override {
            result.page[7] = u8::from(value);
        }
        if let Some(cover) = &values.top_cover {
            result.page[100..104].copy_from_slice(&[
                cover.brightness,
                cover.color.r,
                cover.color.g,
                cover.color.b,
            ]);
        }
        for (at, zone) in [(100, values.logo), (104, values.light_bar)] {
            if let Some(zone) = zone {
                result.page[at..at + 4].copy_from_slice(&[
                    u8::from(zone.enabled),
                    zone.color.r,
                    zone.color.g,
                    zone.color.b,
                ]);
            }
        }
        Ok(result)
    }
    fn require_same_device(&self, other: &Self) -> Result<()> {
        if self.caps != other.caps {
            bail!("灯光设备或能力已变化，拒绝写入旧快照");
        }
        Ok(())
    }
}
fn validate(values: &Values, caps: &Capabilities) -> Result<()> {
    values.validate()?;
    if values.boot_override.is_some() != caps.boot_override
        || values.logo.is_some() != caps.logo
        || values.light_bar.is_some() != caps.light_bar
        || values.top_cover.is_some() != caps.top_cover
    {
        bail!("只能修改原厂声明支持的灯光选项");
    }
    Ok(())
}
trait Io: SettingsAccess {
    fn capabilities(&self) -> Result<Capabilities>;
    fn send(&self, command: i32, word: u32) -> Result<()>;
}
fn capture(io: &impl Io) -> Result<Snapshot> {
    let snapshot = capture_raw(io)?;
    snapshot.values()?;
    Ok(snapshot)
}
fn capture_raw(io: &impl Io) -> Result<Snapshot> {
    let page = io.read(2, 0, 256).map_err(|e| anyhow!(e))?;
    if page.len() != 256 {
        bail!("灯光设置页长度无效");
    }
    let snapshot = Snapshot {
        caps: io.capabilities()?,
        page,
    };
    Ok(snapshot)
}
fn profile(current: &Snapshot, baseline: &Option<Snapshot>) -> Result<Profile> {
    Ok(Profile {
        revision: current.revision(),
        current: current.values()?,
        capabilities: current.caps.clone(),
        can_restore: baseline.is_some(),
        initial: baseline.as_ref().map(Snapshot::values).transpose()?,
    })
}
#[derive(Default)]
pub struct Controller {
    baseline: Mutex<Option<Snapshot>>,
}
impl Controller {
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.with_dchu(|io| profile(&capture(io)?, &scope.baseline))
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.with_dchu(|io| transaction::apply(io, &mut scope.baseline, request))
    }
    pub fn restore(&self, hal: &Hal, expected_revision: &str) -> Result<()> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.with_dchu(|io| transaction::restore(io, &scope.baseline, expected_revision))
    }
}
