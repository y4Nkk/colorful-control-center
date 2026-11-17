use super::{Baseline, State, Status, Task};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    pub version: u8,
    pub operation: String,
    pub baseline: Baseline,
    pub state: Option<State>,
    pub pending: bool,
    pub armed: bool,
    pub plan_prepared: bool,
    pub outcome: Option<Status>,
    pub error: Option<String>,
    pub recovery_error: Option<String>,
}
impl Record {
    pub fn prepared(operation: String, baseline: Baseline) -> Result<Self> {
        let record = Self {
            version: 1,
            operation,
            baseline,
            state: None,
            pending: true,
            armed: false,
            plan_prepared: false,
            outcome: None,
            error: None,
            recovery_error: None,
        };
        record.validate()?;
        Ok(record)
    }
    pub fn update(&mut self, task: &Task) {
        self.state = Some(task.state.clone());
        self.armed = true;
        self.plan_prepared = task.plan_prepared;
        self.pending = task.state.active() || task.state.recovery_required();
        self.outcome = (!task.state.active()).then(|| task.intended_status());
        self.error.clone_from(&task.state.error);
        self.recovery_error = None;
    }
    pub fn set_error(&mut self, message: String) {
        self.error = Some(message);
        if let Some(state) = &mut self.state {
            state.error.clone_from(&self.error);
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.operation.len() != 32
            || !self.operation.bytes().all(|v| v.is_ascii_hexdigit())
        {
            bail!("校准恢复记录版本或操作标识无效");
        }
        self.baseline.validate()?;
        if self.plan_prepared && !self.armed {
            bail!("校准电源计划记录缺少接管标记");
        }
        if [&self.error, &self.recovery_error]
            .into_iter()
            .flatten()
            .any(|e| e.len() > 16_384)
        {
            bail!("校准恢复诊断超过限制");
        }
        if self.outcome.is_some_and(|s| {
            matches!(
                s,
                Status::Running | Status::Paused | Status::RecoveryRequired
            )
        }) {
            bail!("校准结束状态无效");
        }
        match &self.state {
            None if self.pending && self.outcome.is_none() && self.error.is_none() => (),
            None if self.outcome == Some(Status::Failed) && self.error.is_some() => (),
            Some(state) => {
                if state.active() && !self.plan_prepared {
                    bail!("运行中的校准缺少电源计划记录");
                }
                if !self.armed {
                    bail!("未接管的校准记录不能包含运行状态");
                }
                if state.error != self.error || state.phase_elapsed_ms > state.elapsed_ms {
                    bail!("校准任务状态与恢复记录不一致");
                }
                let consistent = if state.active() {
                    self.pending && self.outcome.is_none()
                } else if state.recovery_required() {
                    self.pending && self.outcome.is_some()
                } else {
                    !self.pending && self.outcome == Some(state.status)
                };
                if !consistent {
                    bail!("校准任务恢复标志无效");
                }
            }
            _ => bail!("校准任务记录缺少状态"),
        }
        Ok(())
    }
}
pub(crate) trait Store {
    fn load(&self) -> Result<Option<Record>>;
    fn save(&self, record: &Record) -> Result<()>;
}
pub(crate) struct FileStore(pub PathBuf);
pub(crate) fn recover(
    io: &mut dyn super::Access,
    store: &dyn Store,
    mut record: Record,
) -> Result<()> {
    record.validate()?;
    if !record.armed {
        record.pending = false;
        record.outcome = Some(Status::Failed);
        record.set_error("校准尚未接管，已解除准备记录；未发送硬件释放命令".into());
        record.recovery_error = None;
        return store.save(&record);
    }
    let errors = super::engine::release(io, &record.baseline, record.plan_prepared);
    record.pending = !errors.is_empty();
    let outcome = record.outcome.unwrap_or(Status::Failed);
    record.outcome = Some(outcome);
    if let Some(state) = &mut record.state {
        state.status = if record.pending {
            Status::RecoveryRequired
        } else {
            outcome
        };
    }
    record
        .error
        .get_or_insert_with(|| "校准任务已中断，需释放控制并恢复捕获策略".into());
    record.recovery_error = (!errors.is_empty()).then(|| errors.join("；"));
    if let Some(state) = &mut record.state {
        state.error.clone_from(&record.error);
    }
    store.save(&record)?;
    if record.pending {
        bail!(record.recovery_error.unwrap_or_default());
    }
    Ok(())
}
impl Store for FileStore {
    fn load(&self) -> Result<Option<Record>> {
        let file = match std::fs::File::open(&self.0) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take(65_537).read_to_end(&mut bytes)?;
        if bytes.len() > 65_536 {
            bail!("校准记录超过大小限制");
        }
        let record: Record =
            serde_json::from_slice(&bytes).context("校准恢复记录不能解析，已保留原文件")?;
        record.validate()?;
        Ok(Some(record))
    }
    fn save(&self, record: &Record) -> Result<()> {
        record.validate()?;
        let parent = self.0.parent().context("校准记录目录无效")?;
        std::fs::create_dir_all(parent)?;
        crate::storage::write(&self.0, &serde_json::to_vec_pretty(record)?)?;
        Ok(())
    }
}
