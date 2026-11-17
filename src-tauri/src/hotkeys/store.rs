use super::model::*;
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

#[derive(Clone)]
pub struct Loaded {
    pub record: Record,
    pub revision: String,
}
pub(super) struct Inspection {
    pub loaded: Option<Loaded>,
    pub error: Option<String>,
    pub reset_revision: Option<String>,
}
pub(super) fn inspect(path: &Path) -> Result<Inspection> {
    match load(path) {
        Ok(loaded) => Ok(Inspection {
            loaded: Some(loaded),
            error: None,
            reset_revision: None,
        }),
        Err(error) => Ok(Inspection {
            loaded: None,
            error: Some(format!("{error:#}")),
            reset_revision: Some(raw_revision(path)?),
        }),
    }
}
fn raw_revision(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).context("读取不可用热键策略摘要失败")?;
    let mut digest = Sha256::new();
    let mut chunk = [0; 8192];
    loop {
        let count = file.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        digest.update(&chunk[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
pub(super) fn reset(path: &Path, expected: &str) -> Result<()> {
    let inspection = inspect(path)?;
    if inspection.loaded.is_some() || inspection.reset_revision.as_deref() != Some(expected) {
        bail!("不可用热键策略已变化，请重新查看后确认清除");
    }
    crate::storage::write(path, &serde_json::to_vec_pretty(&Record::default())?)
        .context("清除不可用热键策略失败")
}
pub fn load(path: &Path) -> Result<Loaded> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Loaded {
                record: Record::default(),
                revision: "missing".into(),
            });
        }
        Err(error) => return Err(error).context("读取热键策略失败"),
    };
    let mut bytes = Vec::new();
    file.take(65_537).read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        bail!("热键策略文件过大，未覆盖原文件");
    }
    let record: Record = serde_json::from_slice(&bytes).context("热键策略损坏，未覆盖原文件")?;
    record.validate()?;
    Ok(Loaded {
        record,
        revision: format!("{:x}", Sha256::digest(bytes)),
    })
}
pub fn replace(path: &Path, expected: &Loaded, target: &Record) -> Result<Loaded> {
    target.validate()?;
    if load(path)?.revision != expected.revision {
        bail!("热键策略已被修改，请重新读取确认");
    }
    std::fs::create_dir_all(path.parent().context("热键策略目录无效")?)?;
    crate::storage::write(path, &serde_json::to_vec_pretty(target)?).context("保存热键策略失败")?;
    load(path)
}
pub(super) fn bind_mode_driver(path: &Path, approved: &mut Record, identity: &str) -> Result<()> {
    if !approved.settings.enabled || !approved.settings.actions.contains(&Scope::PowerMode) {
        bail!("模式热键未获得GPU调频许可");
    }
    if approved
        .mode_driver
        .as_deref()
        .is_some_and(|saved| saved != identity)
    {
        bail!("模式循环的GPU设备或驱动能力已变化，未替换原授权");
    }
    let loaded = load(path)?;
    if loaded.record != *approved {
        bail!("模式循环的热键许可已撤销");
    }
    if approved.mode_driver.is_none() {
        let mut next = approved.clone();
        next.mode_driver = Some(identity.to_owned());
        *approved = replace(path, &loaded, &next)?.record;
    }
    Ok(())
}
