use super::model::Record;
use anyhow::{Context, Result, bail};
use std::{io::Read, path::PathBuf};

pub struct FileStore(pub PathBuf);
impl FileStore {
    pub fn load(&self) -> Result<Option<Record>> {
        let file = match std::fs::File::open(&self.0) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("读取电池阈值恢复记录"),
        };
        let mut bytes = Vec::new();
        file.take(65_537).read_to_end(&mut bytes)?;
        if bytes.len() > 65_536 {
            bail!("电池阈值恢复记录过大，保留原文件");
        }
        let record: Record =
            serde_json::from_slice(&bytes).context("电池阈值恢复记录损坏，保留原文件")?;
        record.validate()?;
        Ok(Some(record))
    }
    pub fn save(&self, expected: Option<&Record>, next: &Record) -> Result<()> {
        next.validate()?;
        if self.load()?.as_ref() != expected {
            bail!("电池阈值恢复记录已变化，未覆盖");
        }
        std::fs::create_dir_all(self.0.parent().context("电池阈值记录目录无效")?)?;
        crate::storage::write(&self.0, &serde_json::to_vec_pretty(next)?)
            .context("保存电池阈值恢复记录")
    }
    pub fn clear(&self, expected: &Record) -> Result<()> {
        if self.load()?.as_ref() != Some(expected) {
            bail!("电池阈值恢复记录已变化，未删除");
        }
        std::fs::remove_file(&self.0).context("删除已恢复的电池阈值记录")
    }
}
