use super::model::Record;
use anyhow::{Context, Result, bail};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub trait Store {
    fn load(&self) -> Result<Option<Record>>;
    fn save(&self, expected: &Option<Record>, record: &Record) -> Result<()>;
}
pub struct FileStore(pub PathBuf);
impl Store for FileStore {
    fn load(&self) -> Result<Option<Record>> {
        load(&self.0)
    }
    fn save(&self, expected: &Option<Record>, record: &Record) -> Result<()> {
        record.validate()?;
        if self.load()? != *expected {
            bail!("节能恢复记录已变化，未覆盖现有文件");
        }
        let parent = self.0.parent().context("节能记录目录无效")?;
        std::fs::create_dir_all(parent)?;
        let bytes = serde_json::to_vec_pretty(record)?;
        if bytes.len() > 65_536 {
            bail!("节能记录超过大小限制，未替换原文件");
        }
        crate::storage::write(&self.0, &bytes)?;
        Ok(())
    }
}
pub fn load(path: &Path) -> Result<Option<Record>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(65_537).read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        bail!("节能记录超过大小限制");
    }
    let record: Record =
        serde_json::from_slice(&bytes).context("节能记录不能解析，已保留原文件")?;
    record.validate()?;
    Ok(Some(record))
}
