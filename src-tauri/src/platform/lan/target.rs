use super::{model::*, transaction::Plan};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceTarget {
    pub identity: Identity,
    pub name: String,
    pub values: BTreeMap<Field, Option<Value>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub capabilities: Capabilities,
    pub devices: Vec<DeviceTarget>,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        if !self.capabilities.any() || self.devices.is_empty() || self.devices.len() > 256 {
            bail!("LAN档案设备或能力为空、过多");
        }
        let (mut ids, mut drivers) = (BTreeSet::new(), BTreeSet::new());
        for device in &self.devices {
            device.identity.validate()?;
            if device.name.len() > 1024
                || device.values.is_empty()
                || device.values.len() > 3
                || !ids.insert(device.identity.instance.to_ascii_uppercase())
                || !drivers.insert(&device.identity.driver)
            {
                bail!("LAN档案设备、字段或驱动绑定无效");
            }
            for (&field, value) in &device.values {
                if !field.writable()
                    || field == Field::PowerSavingMode && !self.capabilities.power_saving
                    || field != Field::PowerSavingMode && !self.capabilities.wake_patch
                {
                    bail!("LAN档案字段不属于声明能力");
                }
                if let Some(value) = value {
                    value.validate()?;
                }
                if field == Field::PowerSavingMode
                    && !matches!(
                        value.as_ref().and_then(Value::as_string).as_deref(),
                        Some("0" | "1")
                    )
                {
                    bail!("LAN档案节能必须是原厂字符串0或1");
                }
            }
        }
        Ok(())
    }
    pub fn capture(frame: &Frame) -> Result<Option<Self>> {
        frame.validate()?;
        if !frame.capabilities.any() {
            return Ok(None);
        }
        let mut devices = Vec::new();
        for device in &frame.devices {
            let mut values = BTreeMap::new();
            if frame.capabilities.power_saving && device.values[&Field::PowerSavingMode].is_some() {
                if device.power_saving().is_none() {
                    bail!("LAN节能字段未知，无法完整捕获");
                }
                values.insert(
                    Field::PowerSavingMode,
                    device.values[&Field::PowerSavingMode].clone(),
                );
            }
            if frame.capabilities.wake_patch && device.wake_patch_available() {
                values.insert(
                    Field::PnpCapabilities,
                    device.values[&Field::PnpCapabilities].clone(),
                );
                if device.values[&Field::WakeOnSlot]
                    .as_ref()
                    .and_then(Value::as_string)
                    .is_some()
                {
                    values.insert(Field::WakeOnLink, device.values[&Field::WakeOnLink].clone());
                }
            }
            if !values.is_empty() {
                devices.push(DeviceTarget {
                    identity: device.identity.clone(),
                    name: device.name.clone(),
                    values,
                });
            }
        }
        if devices.is_empty() {
            return Ok(None);
        }
        let target = Self {
            capabilities: frame.capabilities,
            devices,
        };
        target.validate()?;
        Ok(Some(target))
    }
    pub fn plans(&self, frame: &Frame) -> Result<Vec<Plan>> {
        self.validate()?;
        frame.validate()?;
        if self.capabilities != frame.capabilities {
            bail!("LAN档案能力已变化");
        }
        self.devices
            .iter()
            .filter_map(|target| {
                let make = || -> Result<Option<Plan>> {
                    let device = frame.device(&target.identity.instance)?;
                    if device.identity != target.identity {
                        bail!("LAN档案驱动身份已变化");
                    }
                    for field in target.values.keys() {
                        if *field == Field::PowerSavingMode && device.power_saving().is_none()
                            || *field != Field::PowerSavingMode && !device.wake_patch_available()
                            || *field == Field::WakeOnLink
                                && device.values[&Field::WakeOnSlot]
                                    .as_ref()
                                    .and_then(Value::as_string)
                                    .is_none()
                        {
                            bail!("LAN档案字段不再符合原厂适用条件");
                        }
                    }
                    let changes = target
                        .values
                        .iter()
                        .filter_map(|(field, target)| {
                            let before = device.values[field].clone();
                            (before != *target).then(|| Change {
                                field: *field,
                                before,
                                target: target.clone(),
                            })
                        })
                        .collect::<Vec<_>>();
                    Ok((!changes.is_empty()).then(|| Plan {
                        expected: device.clone(),
                        changes,
                    }))
                };
                make().transpose()
            })
            .collect()
    }
}
