use super::model::{Record, RECORD_VERSION};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::PathBuf};

#[derive(Clone)]
pub struct Loaded {
    pub record: Option<Record>,
    pub revision: String,
}
fn revision(bytes: Option<&[u8]>) -> String {
    match bytes {
        Some(bytes) => format!("{:x}", Sha256::digest(bytes)),
        None => "missing".into(),
    }
}
/// 只读取版本号：旧版本记录的字段不再解释。
#[derive(Deserialize)]
struct WrittenVersion {
    version: u8,
}
pub trait Store {
    fn load(&self) -> Result<Loaded>;
    fn replace(&self, expected: &Loaded, record: &Record) -> Result<Loaded>;
}
pub struct FileStore(pub PathBuf);
impl Store for FileStore {
    fn load(&self) -> Result<Loaded> {
        let file = match fs::File::open(&self.0) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Loaded {
                    record: None,
                    revision: revision(None),
                });
            }
            Err(error) => return Err(error).context("读取显卡选择记录失败"),
        };
        let mut bytes = Vec::new();
        file.take(4097).read_to_end(&mut bytes)?;
        if bytes.len() > 4096 {
            bail!("显卡选择记录过大，未覆盖原文件");
        }
        if serde_json::from_slice::<WrittenVersion>(&bytes)
            .is_ok_and(|written| written.version < RECORD_VERSION)
        {
            return Ok(Loaded {
                record: None,
                revision: revision(Some(&bytes)),
            });
        }
        let record: Record =
            serde_json::from_slice(&bytes).context("显卡选择记录损坏，未覆盖原文件")?;
        record.validate()?;
        Ok(Loaded {
            record: Some(record),
            revision: revision(Some(&bytes)),
        })
    }
    fn replace(&self, expected: &Loaded, record: &Record) -> Result<Loaded> {
        record.validate()?;
        if self.load()?.revision != expected.revision {
            bail!("显卡选择记录已被更改，请重新读取并确认");
        }
        fs::create_dir_all(self.0.parent().context("显卡选择记录目录无效")?)?;
        let bytes = serde_json::to_vec_pretty(record)?;
        crate::storage::write(&self.0, &bytes).context("保存显卡选择记录失败")?;
        Ok(Loaded {
            record: Some(record.clone()),
            revision: revision(Some(&bytes)),
        })
    }
}
