use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};

pub(crate) const RECORD_NAME: &str = "software-fan-safety.json";
const STOPPED: &str = "软件曲线因故障停止，请手动预览并确认重新应用；后台档案不会自动重启";

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    version: u8,
    active: bool,
    faulted: bool,
}

pub(crate) struct Journal {
    path: Option<PathBuf>,
    observed: Option<Vec<u8>>,
    record: Record,
    issue: Option<String>,
    closing: bool,
}
impl Default for Journal {
    fn default() -> Self {
        Self {
            path: None,
            observed: None,
            record: Record {
                version: 1,
                ..Record::default()
            },
            issue: None,
            closing: false,
        }
    }
}

impl Journal {
    pub(crate) fn open(path: PathBuf) -> Self {
        let mut journal = Self {
            path: Some(path),
            ..Self::default()
        };
        match journal.read() {
            Ok(Some(bytes)) => {
                let parsed = serde_json::from_slice::<Record>(&bytes)
                    .context("软件风扇恢复记录格式无效")
                    .and_then(|record| {
                        ensure!(record.version == 1, "软件风扇恢复记录版本无效");
                        Ok(record)
                    });
                journal.observed = Some(bytes);
                match parsed {
                    Ok(mut record) => {
                        record.faulted |= record.active;
                        journal.record = record;
                    }
                    Err(error) => journal.fail(&error),
                }
            }
            Ok(None) => (),
            Err(error) => journal.fail(&error),
        }
        journal
    }

    pub(crate) fn reason(&self) -> Option<String> {
        self.record.faulted.then(|| {
            self.issue
                .as_ref()
                .map_or_else(|| STOPPED.into(), |issue| format!("{STOPPED}：{issue}"))
        })
    }

    pub(crate) fn active(&self) -> bool {
        self.record.active
    }

    pub(crate) fn allowed(&mut self) -> Result<()> {
        self.session_open()?;
        self.verify()?;
        if let Some(reason) = self.reason() {
            bail!(reason);
        }
        Ok(())
    }

    pub(crate) fn begin(&mut self, manual: bool) -> Result<()> {
        self.session_open()?;
        if manual {
            if let Err(error) = self.verify() {
                self.fail(&error);
            }
        } else {
            self.allowed()?;
        }
        let before = self.record.active;
        self.record.active = true;
        let result = self.persist();
        if result.is_err() {
            self.record.active = before;
        }
        result
    }

    pub(crate) fn latch(&mut self) -> Result<()> {
        self.record.faulted = true;
        self.verify()?;
        self.persist()
    }

    pub(crate) fn stopped(&mut self) -> Result<()> {
        if !self.record.active {
            return Ok(());
        }
        // EC has released the channels even if the durable record cannot be updated.
        self.record.active = false;
        self.verify()?;
        self.persist()
    }

    pub(crate) fn acknowledge(&mut self) -> Result<()> {
        self.session_open()?;
        self.verify()?;
        ensure!(self.record.active, "软件曲线尚未登记运行，未解除故障锁定");
        self.record.faulted = false;
        self.persist()
    }

    pub(crate) fn close(&mut self) {
        self.closing = true;
    }

    pub(crate) fn session_open(&self) -> Result<()> {
        ensure!(!self.closing, "应用正在退出，禁止重新接管软件风扇");
        Ok(())
    }

    fn read(&self) -> Result<Option<Vec<u8>>> {
        let Some(path) = &self.path else {
            #[cfg(test)]
            return Ok(None);
            #[cfg(not(test))]
            bail!("软件风扇恢复记录未绑定");
        };
        match fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("读取{}失败", path.display())),
        }
    }

    fn verify(&mut self) -> Result<()> {
        let result = self.read().and_then(|actual| {
            #[cfg(test)]
            if self.path.is_none() {
                return Ok(());
            }
            ensure!(
                actual == self.observed,
                "软件风扇恢复记录已变化，请手动重新确认"
            );
            Ok(())
        });
        if let Err(error) = &result {
            self.fail(error);
        }
        result
    }

    fn persist(&mut self) -> Result<()> {
        let result = (|| {
            let Some(path) = &self.path else {
                #[cfg(test)]
                return Ok(None);
                #[cfg(not(test))]
                bail!("软件风扇恢复记录未绑定");
            };
            let bytes = serde_json::to_vec(&self.record)?;
            fs::create_dir_all(path.parent().ok_or_else(|| anyhow!("恢复记录缺少父目录"))?)?;
            crate::storage::write(path, &bytes).context("保存软件风扇恢复记录失败")?;
            Ok(Some(bytes))
        })();
        match result {
            Ok(bytes) => {
                self.observed = bytes;
                self.issue = None;
                Ok(())
            }
            Err(error) => {
                self.fail(&error);
                Err(error)
            }
        }
    }

    fn fail(&mut self, error: &anyhow::Error) {
        self.record.faulted = true;
        self.issue = Some(format!("{error:#}"));
    }
}

#[cfg(test)]
#[path = "journal/tests.rs"]
mod tests;
