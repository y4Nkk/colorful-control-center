use crate::hal::beep_audio::Control;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Record {
    pub version: u8,
    pub initial: Control,
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "原厂电池音频恢复记录版本无效");
        self.initial.validate()
    }
}

pub(super) struct Store(pub PathBuf);
impl Store {
    pub fn load(&self) -> Result<Option<Record>> {
        let file = match std::fs::File::open(&self.0) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("读取原厂电池音频恢复记录"),
        };
        let mut bytes = Vec::new();
        file.take(65537).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 65536, "原厂电池音频恢复记录过大，保留原文件");
        let record: Record = serde_json::from_slice(&bytes).context("原厂电池音频恢复记录损坏")?;
        record.validate()?;
        Ok(Some(record))
    }

    pub fn save(&self, expected: Option<&Record>, next: &Record) -> Result<()> {
        next.validate()?;
        ensure!(
            self.load()?.as_ref() == expected,
            "原厂电池音频恢复记录已变化"
        );
        let bytes = serde_json::to_vec(next)?;
        ensure!(bytes.len() <= 65536, "原厂电池音频恢复记录超出长度限制");
        std::fs::create_dir_all(self.0.parent().context("原厂电池音频恢复目录无效")?)?;
        crate::storage::write(&self.0, &bytes).context("保存原厂电池音频恢复记录失败")
    }

    pub fn check(&self, expected: &Record) -> Result<()> {
        ensure!(
            self.load()?.as_ref() == Some(expected),
            "原厂电池音频恢复记录已变化"
        );
        Ok(())
    }

    pub fn clear(&self, expected: &Record) -> Result<()> {
        self.check(expected)?;
        std::fs::remove_file(&self.0).context("清除已恢复的原厂电池音频记录失败")
    }
}
