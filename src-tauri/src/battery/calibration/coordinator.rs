use super::{
    Access, Baseline, State,
    client::Client,
    native::Native,
    store::{FileStore, Record, Store},
};
use crate::{
    battery::{Controller, energy, model::ChargeValues, protocol},
    hal::Hal,
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub revision: String,
    pub task: Option<State>,
    pub worker_running: bool,
    pub control_acquired: bool,
    pub can_start: bool,
    pub can_stop: bool,
    pub recovery_required: bool,
    pub stop_threshold: u8,
    pub restore_charge: ChargeValues,
    pub restore_energy_enabled: Option<bool>,
    pub known_energy_plan: bool,
    pub has_energy_capability: bool,
    pub unavailable_reason: Option<String>,
    pub error: Option<String>,
}
fn revision(baseline: &Baseline, record: &Option<Record>) -> String {
    let saved = record
        .as_ref()
        .map(|r| (&r.operation, r.pending, r.outcome));
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(baseline, saved)).expect("固定校准结构"))
    )
}
fn observe(
    hal: &Hal,
    energy_path: &Path,
    path: &Path,
    session: &Option<Client>,
) -> Result<Option<Profile>> {
    let record = FileStore(path.into()).load()?;
    let pending = record.as_ref().is_some_and(|r| r.pending);
    // Stop/recovery only needs the prepared record; a failed foreground getter must not hide it.
    let supported =
        pending || hal.with_dchu(|io| protocol::utility_supported(&io.read_bios_capabilities(0, 15)?))?;
    if !supported && record.is_none() {
        return Ok(None);
    }
    let baseline = match record.as_ref().filter(|r| r.pending) {
        Some(record) => record.baseline.clone(),
        None => energy::capture_calibration(hal, energy_path)?,
    };
    let ready = if pending {
        Ok(())
    } else if !supported {
        Err(anyhow::anyhow!("原厂校准能力已变化，请先恢复未完成操作"))
    } else {
        Native::new(hal, energy_path.into()).and_then(|mut io| {
            io.ready()?;
            io.facts()?.before_start()
        })
    };
    let task = session
        .as_ref()
        .and_then(|c| c.state.clone())
        .or_else(|| record.as_ref().and_then(|r| r.state.clone()));
    let can_stop = pending && session.is_some() && task.as_ref().is_none_or(State::active);
    let recovery_required =
        pending && (session.is_none() || task.as_ref().is_some_and(State::recovery_required));
    Ok(Some(Profile {
        revision: revision(&baseline, &record),
        task,
        worker_running: session.is_some(),
        control_acquired: record.as_ref().is_some_and(|r| r.armed),
        can_start: !pending && session.is_none() && ready.is_ok(),
        can_stop,
        recovery_required,
        stop_threshold: baseline.stop_threshold,
        restore_charge: baseline.charge.current.clone(),
        restore_energy_enabled: baseline
            .energy
            .as_ref()
            .and_then(|e| e.policy.as_ref().map(|p| p.enabled)),
        known_energy_plan: baseline.energy.as_ref().is_some_and(|e| e.policy.is_some()),
        has_energy_capability: baseline.energy.is_some(),
        unavailable_reason: ready.err().map(|e| format!("{e:#}")),
        error: session.as_ref().and_then(|c| c.error.clone()).or_else(|| {
            record.and_then(|r| match (r.error, r.recovery_error) {
                (Some(cause), Some(recovery)) => Some(format!("{cause}；恢复：{recovery}")),
                (cause, recovery) => cause.or(recovery),
            })
        }),
    }))
}
fn recover_record(hal: &Hal, energy_path: &Path, store: &FileStore, record: Record) -> Result<()> {
    let mut io = Native::recovery(hal, energy_path.into(), &record.operation)?;
    super::store::recover(&mut io, store, record)
}
fn supervise(
    hal: &Hal,
    energy_path: &Path,
    path: &Path,
    session: &mut Option<Client>,
) -> Result<()> {
    let Some(client) = session.as_mut() else {
        return Ok(());
    };
    if client.pulse()? {
        return Ok(());
    }
    let exited = session.take().expect("已有校准会话");
    let store = FileStore(path.into());
    if let Some(mut record) = store.load()?.filter(|r| r.pending) {
        if let Some(error) = &exited.error {
            record.set_error(error.clone());
        }
        recover_record(hal, energy_path, &store, record)?;
    }
    Ok(())
}
pub(in crate::battery) fn charge_lock(energy_path: &Path) -> Result<Option<&'static str>> {
    Ok(
        FileStore(energy_path.with_file_name("battery-calibration.json"))
            .load()?
            .filter(|r| r.pending)
            .map(|_| "电池校准正在接管或尚需恢复，请先停止并完成恢复"),
    )
}
impl Controller {
    pub fn read_calibration(&self, hal: &Hal) -> Result<Option<Profile>> {
        let _gate = hal.begin_write()?;
        let mut session = self
            .calibration
            .lock()
            .map_err(|_| anyhow::anyhow!("校准会话异常"))?;
        let path = self.energy_path.with_file_name("battery-calibration.json");
        supervise(hal, &self.energy_path, &path, &mut session)?;
        observe(hal, &self.energy_path, &path, &session)
    }
    pub fn start_calibration(&self, hal: &Hal, expected: &str) -> Result<()> {
        let _gate = hal.begin_write()?;
        let mut session = self
            .calibration
            .lock()
            .map_err(|_| anyhow::anyhow!("校准会话异常"))?;
        let path = self.energy_path.with_file_name("battery-calibration.json");
        supervise(hal, &self.energy_path, &path, &mut session)?;
        let preview =
            observe(hal, &self.energy_path, &path, &session)?.context("本机不支持电池校准")?;
        if preview.revision != expected || !preview.can_start {
            bail!("校准状态已变化，请重新读取并确认");
        }
        let baseline = energy::capture_calibration(hal, &self.energy_path)?;
        let previous = FileStore(path.clone()).load()?;
        if revision(&baseline, &previous) != expected {
            bail!("校准前的充电或节能策略已变化，请重新预览");
        }
        let operation = format!(
            "{:032x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        );
        let record = Record::prepared(operation.clone(), baseline)?;
        FileStore(path).save(&record)?;
        *session = Some(Client::prepare(operation)?);
        session.as_mut().expect("校准任务已准备").arm()
    }
    pub fn stop_calibration(&self, _hal: &Hal, expected: &str) -> Result<()> {
        let session = self
            .calibration
            .lock()
            .map_err(|_| anyhow::anyhow!("校准会话异常"))?;
        let path = self.energy_path.with_file_name("battery-calibration.json");
        let record = FileStore(path).load()?.context("没有校准任务")?;
        if revision(&record.baseline, &Some(record.clone())) != expected {
            bail!("校准预览已变化");
        }
        session
            .as_ref()
            .context("校准工作进程已退出，请执行恢复")?
            .stop()
    }
    pub fn restore_calibration(&self, hal: &Hal, expected: &str) -> Result<()> {
        let _gate = hal.begin_write()?;
        let mut session = self
            .calibration
            .lock()
            .map_err(|_| anyhow::anyhow!("校准会话异常"))?;
        let path = self.energy_path.with_file_name("battery-calibration.json");
        supervise(hal, &self.energy_path, &path, &mut session)?;
        if session.is_some() {
            bail!("校准工作进程仍在接管，请先停止并等待释放");
        }
        let store = FileStore(path);
        let record = store.load()?.context("没有校准恢复记录")?;
        if revision(&record.baseline, &Some(record.clone())) != expected || !record.pending {
            bail!("校准恢复预览已变化，请重新读取");
        }
        recover_record(hal, &self.energy_path, &store, record)
    }
    pub fn supervise_calibration(&self, hal: &Hal) -> Result<()> {
        let exited = {
            let mut session = self
                .calibration
                .lock()
                .map_err(|_| anyhow::anyhow!("校准会话异常"))?;
            let Some(client) = session.as_mut() else {
                return Ok(());
            };
            if client.pulse()? {
                return Ok(());
            }
            session.take().expect("已有校准会话")
        };
        let _gate = hal.begin_write()?;
        let session = self
            .calibration
            .lock()
            .map_err(|_| anyhow::anyhow!("校准会话异常"))?;
        let store = FileStore(self.energy_path.with_file_name("battery-calibration.json"));
        if session.is_none() {
            if let Some(mut record) = store
                .load()?
                .filter(|r| r.pending && r.operation == exited.operation)
            {
                if let Some(error) = &exited.error {
                    record.set_error(error.clone());
                }
                recover_record(hal, &self.energy_path, &store, record)?;
            }
        }
        Ok(())
    }

    pub(crate) fn release_calibration_on_exit(&self, hal: &Hal) -> Result<()> {
        let _gate = hal.begin_write()?;
        let mut session = self
            .calibration
            .lock()
            .map_err(|_| anyhow::anyhow!("校准会话异常"))?;
        let Some(client) = session.as_mut() else {
            return Ok(());
        };
        client.stop_for_exit();
        let path = self.energy_path.with_file_name("battery-calibration.json");
        while session.is_some() {
            supervise(hal, &self.energy_path, &path, &mut session)?;
            if session.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
        Ok(())
    }
}
