use super::model::*;
use anyhow::{Result, bail};
use std::collections::BTreeSet;

pub trait Io {
    fn inspect(&self, instance: &str) -> Result<Device>;
    fn prepare(&self, expected: &Device, change: &Change) -> Result<()>;
    fn write(&self, identity: &Identity, field: Field, value: &Option<Value>) -> Result<()>;
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub expected: Device,
    pub changes: Vec<Change>,
}
impl Plan {
    pub fn validate(&self) -> Result<()> {
        self.expected.validate()?;
        if self.changes.is_empty() || self.changes.len() > 3 {
            bail!("LAN写集为空或过大");
        }
        let mut fields = BTreeSet::new();
        for change in &self.changes {
            if !change.field.writable()
                || !fields.insert(change.field)
                || change.before != self.expected.values[&change.field]
                || change.before == change.target
            {
                bail!("LAN写集含未授权、重复或过期字段");
            }
            for value in [&change.before, &change.target].into_iter().flatten() {
                value.validate()?;
            }
        }
        Ok(())
    }
}
fn set(io: &impl Io, identity: &Identity, field: Field, value: &Option<Value>) -> Result<()> {
    if io.inspect(&identity.instance)?.identity != *identity {
        bail!("LAN设备或驱动身份已变化");
    }
    io.write(identity, field, value)?;
    let current = io.inspect(&identity.instance)?;
    if current.identity != *identity || current.values[&field] != *value {
        bail!("LAN字段写后回读不一致");
    }
    Ok(())
}
#[derive(Default)]
pub struct Touched(pub Vec<Field>);
impl Touched {
    pub fn dirty(&self) -> bool {
        !self.0.is_empty()
    }
}
pub fn execute(io: &impl Io, plan: &Plan, touched: &mut Touched) -> Result<()> {
    plan.validate()?;
    if io.inspect(&plan.expected.identity.instance)? != plan.expected {
        bail!("LAN设备、驱动或字段已变化，请重新预览");
    }
    let result = (|| -> Result<()> {
        for change in &plan.changes {
            let current = io.inspect(&plan.expected.identity.instance)?;
            if current.identity != plan.expected.identity
                || current.values[&change.field] != change.before
            {
                bail!("LAN目标字段被外部修改");
            }
            io.prepare(&current, change)?;
            touched.0.push(change.field);
            set(io, &plan.expected.identity, change.field, &change.target)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let recovery = rollback(io, plan, touched);
        bail!(
            "LAN应用失败：{error:#}；{}",
            if recovery.is_ok() {
                "本次前态未变化或已补偿".into()
            } else {
                format!("补偿失败：{:#}", recovery.unwrap_err())
            }
        );
    }
    Ok(())
}
pub fn restore(io: &impl Io, plan: &Plan, touched: &mut Touched) -> Result<()> {
    plan.validate()?;
    if io.inspect(&plan.expected.identity.instance)? != plan.expected {
        bail!("LAN恢复预览已变化");
    }
    let mut failures = Vec::new();
    for change in &plan.changes {
        touched.0.push(change.field);
        if let Err(error) = set(io, &plan.expected.identity, change.field, &change.target) {
            failures.push(format!("{}：{error:#}", change.field.name()));
        }
    }
    if !failures.is_empty() {
        bail!("LAN初始恢复未完成：{}", failures.join("；"));
    }
    Ok(())
}
pub fn rollback(io: &impl Io, plan: &Plan, touched: &mut Touched) -> Result<()> {
    let mut failures = Vec::new();
    for field in touched.0.clone().into_iter().rev() {
        let change = plan
            .changes
            .iter()
            .find(|change| change.field == field)
            .unwrap();
        let result = (|| -> Result<()> {
            let current = io.inspect(&plan.expected.identity.instance)?;
            if current.identity != plan.expected.identity {
                bail!("LAN补偿驱动身份已变化");
            }
            if current.values[&field] != change.before && current.values[&field] != change.target {
                bail!("LAN补偿字段已被外部修改，保留外部值");
            }
            set(io, &plan.expected.identity, field, &change.before)
        })();
        match result {
            Ok(()) => touched.0.retain(|owned| *owned != field),
            Err(error) => failures.push(format!("{}：{error:#}", field.name())),
        }
    }
    if !failures.is_empty() {
        bail!(failures.join("；"));
    }
    Ok(())
}
