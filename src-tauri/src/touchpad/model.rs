use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub devices: Vec<String>,
    pub enabled: Option<bool>,
    pub saved: u8,
}
impl Snapshot {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.devices.len() > 128 {
            bail!("触摸板识别设备超过支持数量");
        }
        let mut previous: Option<&str> = None;
        for id in &self.devices {
            if id.is_empty()
                || id.len() > 4096
                || id.contains('\0')
                || previous.is_some_and(|old| old >= id.as_str())
            {
                bail!("触摸板设备身份为空、重复或顺序无效");
            }
            previous = Some(id);
        }
        Ok(())
    }
    pub(crate) fn available(&self) -> Result<bool> {
        self.validate()?;
        if self.devices.is_empty() {
            bail!("未识别原厂I2C触摸板控制条件");
        }
        self.enabled.context("Windows触摸板状态未知，不能切换")
    }
    pub(crate) fn same_devices(&self, other: &Self) -> Result<()> {
        self.available()?;
        other.available()?;
        if self.devices != other.devices {
            bail!("触摸板设备集合已变化，原操作记录保留；连接原设备后重试");
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Result<String> {
        self.available()?;
        digest(&self.devices)
    }
    pub(crate) fn revision(&self, initial: &Option<Self>) -> Result<String> {
        digest(&(self, initial))
    }
    pub(crate) fn target(&self, target: &Target) -> Result<Self> {
        target.validate()?;
        let after = Self {
            devices: target.devices.clone(),
            enabled: Some(target.enabled),
            saved: u8::from(target.saved_enabled),
        };
        self.same_devices(&after)?;
        Ok(after)
    }
}
fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub devices: Vec<String>,
    pub enabled: bool,
    pub saved_enabled: bool,
}
impl Target {
    pub(crate) fn validate(&self) -> Result<()> {
        Snapshot {
            devices: self.devices.clone(),
            enabled: Some(self.enabled),
            saved: u8::from(self.saved_enabled),
        }
        .available()
        .map(|_| ())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub enabled: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub revision: String,
    pub current: Snapshot,
    pub initial: Option<Snapshot>,
    pub can_restore: bool,
    pub restore_blocked_reason: Option<String>,
    pub blocked_reason: Option<String>,
}
