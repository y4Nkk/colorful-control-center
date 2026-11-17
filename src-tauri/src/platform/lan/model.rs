use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const CLASS: &str = "{4d36e972-e325-11ce-bfc1-08002be10318}";
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    PowerSavingMode,
    PnpCapabilities,
    WakeOnLink,
    S5WakeOnLan,
    ShutdownWake,
    WakeOnSlot,
}
impl Field {
    pub fn name(self) -> &'static str {
        match self {
            Self::PowerSavingMode => "PowerSavingMode",
            Self::PnpCapabilities => "PnPCapabilities",
            Self::WakeOnLink => "WakeOnLink",
            Self::S5WakeOnLan => "S5WakeOnLan",
            Self::ShutdownWake => "ShutdownWake",
            Self::WakeOnSlot => "WakeOnSlot",
        }
    }
    pub fn writable(self) -> bool {
        matches!(
            self,
            Self::PowerSavingMode | Self::PnpCapabilities | Self::WakeOnLink
        )
    }
}
pub const FIELDS: [Field; 6] = [
    Field::PowerSavingMode,
    Field::PnpCapabilities,
    Field::WakeOnLink,
    Field::S5WakeOnLan,
    Field::ShutdownWake,
    Field::WakeOnSlot,
];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Value {
    pub kind: u32,
    pub bytes: Vec<u8>,
}
impl Value {
    pub fn string(text: &str) -> Self {
        Self {
            kind: 1,
            bytes: text
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect(),
        }
    }
    pub fn dword(value: u32) -> Self {
        Self {
            kind: 4,
            bytes: value.to_le_bytes().to_vec(),
        }
    }
    pub fn as_string(&self) -> Option<String> {
        if self.kind != 1 || self.bytes.len() < 2 || self.bytes.len() % 2 != 0 {
            return None;
        }
        let mut chars: Vec<_> = self
            .bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect();
        if chars.pop() != Some(0) || chars.contains(&0) {
            return None;
        }
        String::from_utf16(&chars).ok()
    }
    pub fn validate(&self) -> Result<()> {
        if self.bytes.len() > 4096 {
            bail!("LAN驱动字段超过4096字节");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Identity {
    pub instance: String,
    pub driver: String,
    pub net_cfg_id: Option<Value>,
    pub driver_version: Option<Value>,
    pub matching_id: Option<Value>,
}
impl Identity {
    pub fn validate(&self) -> Result<()> {
        let suffix = self
            .driver
            .strip_prefix(&format!("{CLASS}\\"))
            .context("LAN驱动类不匹配")?;
        if suffix.len() != 4
            || !suffix.bytes().all(|v| v.is_ascii_digit())
            || self.instance.is_empty()
            || self.instance.len() > 512
            || self.instance.contains('\0')
        {
            bail!("LAN设备身份无效");
        }
        for value in [&self.net_cfg_id, &self.driver_version, &self.matching_id]
            .into_iter()
            .flatten()
        {
            value.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Device {
    pub identity: Identity,
    pub name: String,
    pub values: BTreeMap<Field, Option<Value>>,
}
impl Device {
    pub fn validate(&self) -> Result<()> {
        self.identity.validate()?;
        if self.name.len() > 1024
            || self.values.len() != FIELDS.len()
            || !FIELDS.iter().all(|field| self.values.contains_key(field))
        {
            bail!("LAN设备字段集合无效");
        }
        for value in self.values.values().flatten() {
            value.validate()?;
        }
        Ok(())
    }
    pub fn power_saving(&self) -> Option<bool> {
        match self
            .values
            .get(&Field::PowerSavingMode)?
            .as_ref()?
            .as_string()?
            .as_str()
        {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        }
    }
    pub fn wake_patch_available(&self) -> bool {
        [Field::S5WakeOnLan, Field::ShutdownWake, Field::WakeOnSlot]
            .into_iter()
            .any(|field| {
                self.values
                    .get(&field)
                    .and_then(Option::as_ref)
                    .and_then(Value::as_string)
                    .is_some()
            })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    pub power_saving: bool,
    pub wake_patch: bool,
}
impl Capabilities {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 24 {
            bail!("LAN能力页长度无效");
        }
        match &bytes[..2] {
            [0, 0] => Ok(Self::default()),
            [1, 0] => Ok(Self {
                power_saving: bytes[21] & 128 != 0,
                wake_patch: bytes[6] & 32 != 0,
            }),
            _ => bail!("未知LAN能力页版本"),
        }
    }
    pub fn any(self) -> bool {
        self.power_saving || self.wake_patch
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Frame {
    pub capabilities: Capabilities,
    pub recommendation: Option<bool>,
    pub devices: Vec<Device>,
}
impl Frame {
    pub fn validate(&self) -> Result<()> {
        let (mut instances, mut drivers) = (BTreeSet::new(), BTreeSet::new());
        for device in &self.devices {
            device.validate()?;
            if !instances.insert(device.identity.instance.to_ascii_uppercase())
                || !drivers.insert(device.identity.driver.clone())
            {
                bail!("LAN设备或驱动绑定重复");
            }
        }
        if self.devices.len() > 256 {
            bail!("LAN设备过多");
        }
        Ok(())
    }
    pub fn device(&self, instance: &str) -> Result<&Device> {
        self.devices
            .iter()
            .find(|value| value.identity.instance == instance)
            .context("原LAN设备已移除")
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub device_id: String,
    pub power_saving: Option<bool>,
    pub apply_wake_patch: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Change {
    pub field: Field,
    pub before: Option<Value>,
    pub target: Option<Value>,
}
pub fn changes(frame: &Frame, request: &Request) -> Result<Vec<Change>> {
    frame.validate()?;
    let device = frame.device(&request.device_id)?;
    let mut targets = BTreeMap::new();
    if let Some(target) = request.power_saving {
        if !frame.capabilities.power_saving || device.power_saving().is_none() {
            bail!("LAN节能字段未知或未声明能力");
        }
        targets.insert(
            Field::PowerSavingMode,
            Some(Value::string(if target { "1" } else { "0" })),
        );
    }
    if request.apply_wake_patch {
        if !frame.capabilities.wake_patch || !device.wake_patch_available() {
            bail!("设备不符合原厂LAN唤醒补丁条件");
        }
        targets.insert(Field::PnpCapabilities, Some(Value::dword(256)));
        if device.values[&Field::WakeOnSlot]
            .as_ref()
            .and_then(Value::as_string)
            .is_some()
        {
            targets.insert(Field::WakeOnLink, Some(Value::string("2")));
        }
    }
    if request.power_saving.is_none() && !request.apply_wake_patch {
        bail!("请选择LAN策略目标");
    }
    Ok(targets
        .into_iter()
        .filter_map(|(field, target)| {
            let before = device.values[&field].clone();
            (before != target).then_some(Change {
                field,
                before,
                target,
            })
        })
        .collect())
}
pub fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

// 建议仅填草稿，不是已确认目标的执行权威。
pub fn revision(frame: &Frame, record: &impl Serialize) -> Result<String> {
    let mut source = frame.clone();
    source.recommendation = None;
    digest(&(&source, record))
}
