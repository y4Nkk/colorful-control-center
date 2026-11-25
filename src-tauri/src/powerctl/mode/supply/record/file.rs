use super::{RECORD_VERSION, Record};
use anyhow::{Context, Error, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{ErrorKind, Read},
    path::PathBuf,
};

const MAX_SIZE: usize = 262_144;

pub(in crate::powerctl::mode::supply) enum Inspection {
    Missing,
    Current(Record),
    Invalid { error: Error, revision: String },
}

#[derive(Clone)]
pub(in crate::powerctl::mode::supply) struct FileStore(pub PathBuf);

impl FileStore {
    pub fn load(&self) -> Result<Option<Record>> {
        match self.inspect()? {
            Inspection::Missing => Ok(None),
            Inspection::Current(record) => Ok(Some(record)),
            Inspection::Invalid { error, .. } => Err(error),
        }
    }

    pub fn inspect(&self) -> Result<Inspection> {
        let mut file = match fs::File::open(&self.0) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Inspection::Missing),
            Err(error) => return Err(error).context("读取供电模式恢复记录"),
        };
        let mut bytes = Vec::new();
        (&mut file)
            .take((MAX_SIZE + 1) as u64)
            .read_to_end(&mut bytes)
            .context("读取供电模式恢复记录内容")?;
        match decode(&bytes) {
            Ok(record) => Ok(Inspection::Current(record)),
            Err(error) => Ok(Inspection::Invalid {
                error,
                revision: raw_revision(&bytes, &mut file)?,
            }),
        }
    }

    pub fn archive_invalid(
        &self,
        expected: &str,
        pause: impl FnOnce() -> Result<()>,
    ) -> Result<PathBuf> {
        let Inspection::Invalid { revision, .. } = self.inspect()? else {
            bail!("当前没有可归档的无效供电记录，请重新读取");
        };
        if revision != expected {
            bail!("无效供电记录已变化，请重新读取后确认归档");
        }
        let parent = self.0.parent().context("供电模式记录目录无效")?;
        let name = self.0.file_name().context("供电模式记录文件名无效")?;
        let archive = tempfile::Builder::new()
            .prefix("oem-supply-archive-")
            .tempdir_in(parent)
            .context("创建供电记录归档目录")?;
        let destination = archive.path().join(name);
        pause()?;
        fs::rename(&self.0, &destination).context("归档无效供电记录，原文件仍保留")?;
        Ok(archive.keep().join(name))
    }

    pub fn save(&self, expected: Option<&Record>, next: &Record) -> Result<()> {
        next.validate()?;
        if self.load()?.as_ref() != expected {
            bail!("供电模式恢复记录已变化");
        }
        fs::create_dir_all(self.0.parent().context("供电模式记录目录无效")?)?;
        crate::storage::write(&self.0, &serde_json::to_vec_pretty(next)?)?;
        Ok(())
    }

    pub fn clear(&self, expected: &Record) -> Result<()> {
        if self.load()?.as_ref() != Some(expected) {
            bail!("供电模式恢复记录已变化");
        }
        fs::remove_file(&self.0).context("删除供电模式恢复记录")
    }
}

fn decode(bytes: &[u8]) -> Result<Record> {
    if bytes.len() > MAX_SIZE {
        bail!("供电模式恢复记录过大");
    }
    let record: Record = serde_json::from_slice(bytes)
        .with_context(|| format!("供电模式恢复记录不符合当前版本 {RECORD_VERSION}"))?;
    record.validate()?;
    Ok(record)
}

fn raw_revision(prefix: &[u8], file: &mut fs::File) -> Result<String> {
    let mut digest = Sha256::new();
    digest.update(prefix);
    let mut buffer = [0; 8192];
    loop {
        let length = file.read(&mut buffer).context("读取无效供电记录摘要")?;
        if length == 0 {
            return Ok(format!("{:x}", digest.finalize()));
        }
        digest.update(&buffer[..length]);
    }
}
