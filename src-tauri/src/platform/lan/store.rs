use super::{model::*, transaction::Plan};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Initial {
    pub identity: Identity,
    pub fields: BTreeMap<Field, Option<Value>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub version: u32,
    pub devices: Vec<Initial>,
    pub last_error: Option<String>,
}
impl Default for Record {
    fn default() -> Self {
        Self {
            version: 1,
            devices: vec![],
            last_error: None,
        }
    }
}
impl Record {
    pub fn encoded(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        if bytes.len() > MAX_RECORD_BYTES {
            bail!("LAN恢复记录过大，未覆盖或写入系统");
        }
        Ok(bytes)
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.devices.len() > 256
            || self
                .last_error
                .as_ref()
                .is_some_and(|error| error.len() > 65_536)
        {
            bail!("LAN恢复记录版本或大小无效");
        }
        let (mut identities, mut drivers) = (BTreeSet::new(), BTreeSet::new());
        for initial in &self.devices {
            initial.identity.validate()?;
            if !identities.insert(&initial.identity.instance)
                || !drivers.insert(&initial.identity.driver)
                || initial.fields.is_empty()
                || initial.fields.keys().any(|field| !field.writable())
            {
                bail!("LAN恢复记录设备或字段无效");
            }
            for value in initial.fields.values().flatten() {
                value.validate()?;
            }
        }
        Ok(())
    }
    pub fn remember(&mut self, plan: &Plan) -> Result<()> {
        plan.validate()?;
        self.validate()?;
        let index = if let Some(index) = self
            .devices
            .iter()
            .position(|value| value.identity.instance == plan.expected.identity.instance)
        {
            if self.devices[index].identity != plan.expected.identity {
                bail!("LAN驱动身份与初始恢复记录不一致");
            }
            index
        } else {
            self.devices.push(Initial {
                identity: plan.expected.identity.clone(),
                fields: BTreeMap::new(),
            });
            self.devices.len() - 1
        };
        for change in &plan.changes {
            self.devices[index]
                .fields
                .entry(change.field)
                .or_insert_with(|| change.before.clone());
        }
        self.validate()
    }
    pub fn restore_plan(&self, device: &Device) -> Result<Option<Plan>> {
        self.validate()?;
        device.validate()?;
        let initial = self
            .devices
            .iter()
            .find(|value| value.identity.instance == device.identity.instance)
            .context("没有该LAN设备的初始记录")?;
        if initial.identity != device.identity {
            bail!("LAN原驱动已变化，保留恢复记录并拒绝写入新驱动");
        }
        let changes = initial
            .fields
            .iter()
            .filter_map(|(field, target)| {
                let before = device.values[field].clone();
                (before != *target).then_some(Change {
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
    }
}
pub fn read(path: &Path) -> Result<Record> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Record::default()),
        Err(error) => return Err(error.into()),
    };
    if bytes.len() > MAX_RECORD_BYTES {
        bail!("LAN恢复记录过大");
    }
    let record: Record = serde_json::from_slice(&bytes).context("LAN恢复记录损坏，未覆盖")?;
    record.validate()?;
    Ok(record)
}
pub fn write(path: &Path, expected: &Record, next: &Record) -> Result<()> {
    let bytes = next.encoded()?;
    if read(path)? != *expected {
        bail!("LAN恢复文件被外部修改，未覆盖或写入系统");
    }
    crate::storage::write(path, &bytes).context("保存LAN初始恢复记录")
}
