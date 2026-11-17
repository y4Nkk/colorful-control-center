use super::model::Record;
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{io::Read, path::PathBuf};
#[derive(Clone)]
pub struct Loaded {
    pub record: Option<Record>,
    pub revision: String,
}
pub trait Store {
    fn load(&self) -> Result<Loaded>;
    fn replace(&self, expected: &Loaded, record: &Record) -> Result<Loaded>;
}
pub struct FileStore(pub PathBuf);
impl Store for FileStore {
    fn load(&self) -> Result<Loaded> {
        let file = match std::fs::File::open(&self.0) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Loaded {
                    record: None,
                    revision: "missing".into(),
                });
            }
            Err(error) => return Err(error).context("无法读取宏恢复记录"),
        };
        let mut bytes = Vec::new();
        file.take(4_194_305).read_to_end(&mut bytes)?;
        if bytes.len() > 4_194_304 {
            bail!("宏恢复记录过大，未覆盖原文件");
        }
        let record: Record =
            serde_json::from_slice(&bytes).context("宏恢复记录损坏，未覆盖原文件")?;
        record.validate()?;
        Ok(Loaded {
            record: Some(record),
            revision: format!("{:x}", Sha256::digest(&bytes)),
        })
    }
    fn replace(&self, expected: &Loaded, record: &Record) -> Result<Loaded> {
        record.validate()?;
        if self.load()?.revision != expected.revision {
            bail!("宏恢复记录已变化，请重新读取并确认");
        }
        std::fs::create_dir_all(self.0.parent().context("宏记录目录无效")?)?;
        let bytes = serde_json::to_vec_pretty(record)?;
        if bytes.len() > 4_194_304 {
            bail!("宏恢复记录超过保存限制");
        }
        crate::storage::write(&self.0, &bytes).context("保存宏恢复记录失败")?;
        Ok(Loaded {
            record: Some(record.clone()),
            revision: format!("{:x}", Sha256::digest(&bytes)),
        })
    }
}
