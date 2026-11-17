use super::model::Record;
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

pub(super) fn invalid_revision(path: &Path) -> Result<Option<String>> {
    if read(path).is_ok() {
        return Ok(None);
    }
    let bytes = fs::read(path).context("读取不可用自动档案规则摘要失败")?;
    Ok(Some(format!("{:x}", Sha256::digest(bytes))))
}

pub(super) fn reset(path: &Path, expected: &str) -> Result<Store> {
    if invalid_revision(path)?.as_deref() != Some(expected) {
        bail!("不可用自动档案规则已变化，请重新读取后确认清除");
    }
    crate::storage::write(path, &serde_json::to_vec_pretty(&Record::default())?)
        .context("清除不可用自动档案规则失败，保留原文件")?;
    Store::open(path.to_owned())
}

pub(super) fn read(path: &Path) -> Result<Option<Record>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("读取自动档案策略失败"),
    };
    let record: Record =
        serde_json::from_slice(&bytes).context("自动档案策略文件无效，保留原文件")?;
    record.validate()?;
    Ok(Some(record))
}

pub(super) struct Store {
    pub path: PathBuf,
    current: Option<Record>,
    initial: Option<Option<Record>>,
}
impl Store {
    pub fn bind_mode_driver(
        &mut self,
        expected: &Record,
        trigger: super::model::Trigger,
        facts: &crate::powerctl::mode::profile::ApprovalPreview,
    ) -> Result<Record> {
        self.check()?;
        if self.record() != *expected {
            bail!("自动档案许可已变化，未记录GPU驱动");
        }
        let mut next = expected.clone();
        let binding = next.binding_mut(trigger).context("自动档案规则已移除")?;
        facts.check(binding.mode.as_ref().context("自动档案缺少完整模式授权")?)?;
        let identity = &facts
            .clocks
            .driver
            .as_ref()
            .context("未取得实际GPU驱动身份")?
            .identity;
        if binding.bind_driver(identity)? {
            crate::storage::write(&self.path, &serde_json::to_vec_pretty(&next)?)
                .context("记录自动档案GPU驱动失败，未执行相关目标")?;
            self.current = Some(next.clone());
        }
        Ok(next)
    }
    pub fn open(path: PathBuf) -> Result<Self> {
        let current = read(&path)?;
        Ok(Self {
            path,
            current,
            initial: None,
        })
    }
    pub fn record(&self) -> Record {
        self.current.clone().unwrap_or_default()
    }
    pub fn check(&self) -> Result<()> {
        if read(&self.path)? != self.current {
            bail!("自动档案策略已被外部修改，请重新读取");
        }
        Ok(())
    }
    pub fn save(&mut self, expected: &Record, target: &Record) -> Result<()> {
        self.check()?;
        if *expected != self.record() {
            bail!("自动档案预览已变化，请重新确认");
        }
        target.validate()?;
        if self.current.as_ref() == Some(target) {
            return Ok(());
        }
        let bytes = serde_json::to_vec_pretty(target)?;
        crate::storage::write(&self.path, &bytes).context("保存自动档案策略失败，保留原文件")?;
        self.initial.get_or_insert_with(|| self.current.clone());
        self.current = Some(target.clone());
        Ok(())
    }
    pub fn initial_record(&self) -> Option<Record> {
        self.initial
            .as_ref()
            .map(|record| record.clone().unwrap_or_default())
    }
    pub fn reload(&mut self) -> Result<()> {
        self.current = read(&self.path)?;
        Ok(())
    }
    pub fn restore(&mut self, expected: &Record) -> Result<()> {
        self.check()?;
        if *expected != self.record() {
            bail!("自动档案预览已变化，请重新确认");
        }
        let original = self
            .initial
            .as_ref()
            .context("没有可恢复的初始自动档案策略")?;
        match original {
            Some(record) => crate::storage::write(&self.path, &serde_json::to_vec_pretty(record)?)?,
            None => match fs::remove_file(&self.path) {
                Ok(()) => (),
                Err(error) if error.kind() == ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            },
        }
        self.current = original.clone();
        self.initial = None;
        Ok(())
    }
}
