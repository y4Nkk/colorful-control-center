use super::Hal;
use anyhow::{Result, anyhow, ensure};
use std::{path::PathBuf, sync::Mutex};
pub(super) mod journal;
pub(crate) use journal::RECORD_NAME;

impl Hal {
    pub fn safety_status(&self) -> String {
        let status = self
            .safety
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "监护异常".into());
        match self.software_fan_safety.lock() {
            Ok(journal) if journal.active() && !self.software_fan_supported() => {
                format!(
                    "软件风扇归还未证实。{}",
                    super::firmware::FAN_CONTROL_UNVERIFIED
                )
            }
            Ok(journal) => match journal.reason() {
                Some(reason) if !status.contains(&reason) => format!("{status}；{reason}"),
                _ => status,
            },
            Err(_) => "软件风扇恢复记录状态异常".into(),
        }
    }

    pub(crate) fn bind_software_fan_record(&mut self, path: PathBuf) {
        self.software_fan_safety = Mutex::new(journal::Journal::open(path));
    }

    pub(crate) fn latch_software_fan_fault(&self) {
        let result = self
            .software_fan_safety
            .lock()
            .map_err(|_| anyhow!("软件风扇恢复记录状态异常"))
            .and_then(|mut journal| journal.latch());
        if let Err(error) = result {
            log::error!("[software-fan] 故障登记：{error:#}");
        }
    }

    pub(crate) fn ensure_software_fan_restart_allowed(&self) -> Result<()> {
        self.ensure_software_fan_start_allowed(false)
    }

    pub(crate) fn ensure_software_fan_start_allowed(&self, manual: bool) -> Result<()> {
        let mut journal = self
            .software_fan_safety
            .lock()
            .map_err(|_| anyhow!("软件风扇恢复记录状态异常"))?;
        if manual {
            journal.session_open()
        } else {
            journal.allowed()
        }
    }

    pub(crate) fn close_software_fan_session(&self) -> Result<()> {
        self.software_fan_safety
            .lock()
            .map_err(|_| anyhow!("软件风扇恢复记录状态异常"))?
            .close();
        Ok(())
    }

    pub(crate) fn record_software_fan_started(&self, manual: bool) -> Result<()> {
        self.software_fan_safety
            .lock()
            .map_err(|_| anyhow!("软件风扇恢复记录状态异常"))?
            .begin(manual)
    }

    pub(crate) fn record_software_fan_stopped(&self) -> Result<()> {
        self.software_fan_safety
            .lock()
            .map_err(|_| anyhow!("软件风扇恢复记录状态异常"))?
            .stopped()
    }

    pub(crate) fn software_fan_release_unverified(&self) -> Result<bool> {
        Ok(self
            .software_fan_safety
            .lock()
            .map_err(|_| anyhow!("软件风扇恢复记录状态异常"))?
            .active())
    }

    pub(crate) fn acknowledge_software_fan_restart(&self) -> Result<()> {
        let progress = self
            .software_progress
            .lock()
            .map_err(|_| anyhow!("软件曲线进度异常"))?;
        ensure!(progress.is_some(), "软件曲线已停止，未解除故障锁定");
        self.software_fan_safety
            .lock()
            .map_err(|_| anyhow!("软件风扇恢复记录状态异常"))?
            .acknowledge()
    }
}
