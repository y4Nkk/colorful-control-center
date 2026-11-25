use super::{ProfileFile, ProfileGroup, ProfileMeta, authorization, profile_path, validate_file};
use anyhow::{Context, Result, bail};
use std::{
    collections::HashMap,
    fs, mem,
    path::{Path, PathBuf},
};

const MAX_CACHED_PROFILES: usize = 256;
const MAX_CACHED_BYTES: usize = 4 * 1024 * 1024;

struct CachedProfile {
    bytes: Vec<u8>,
    meta: ProfileMeta,
}

#[derive(Default)]
pub(super) struct Listing {
    profiles: HashMap<PathBuf, CachedProfile>,
}

impl Listing {
    pub(super) fn remove(&mut self, path: &Path) {
        self.profiles.remove(path);
    }

    pub(super) fn read(&mut self, dir: &Path) -> Result<Vec<ProfileMeta>> {
        let mut previous = mem::take(&mut self.profiles);
        let mut files = Vec::new();
        let mut cached_bytes = 0;
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path).with_context(|| format!("读取 {} 失败", path.display()))?;
            // 外部改写可能保留文件大小和修改时间，复用时必须逐字节相等。
            let profile = match previous.remove(&path) {
                Some(cached) if cached.bytes == bytes => cached,
                _ => CachedProfile {
                    meta: read_metadata(dir, &path, &bytes)?,
                    bytes,
                },
            };
            let size = profile.bytes.capacity();
            if self.profiles.len() < MAX_CACHED_PROFILES && size <= MAX_CACHED_BYTES - cached_bytes
            {
                cached_bytes += size;
                files.push(profile.meta.clone());
                self.profiles.insert(path, profile);
            } else {
                files.push(profile.meta);
            }
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }
}

fn read_metadata(dir: &Path, path: &Path, bytes: &[u8]) -> Result<ProfileMeta> {
    let file: ProfileFile =
        serde_json::from_slice(bytes).with_context(|| format!("档案 {} 损坏", path.display()))?;
    validate_file(&file)?;
    if profile_path(dir, &file.name)? != path {
        bail!("档案名与文件名不一致");
    }
    let revision = authorization::digest(&file)?;
    Ok(ProfileMeta {
        groups: ProfileGroup::captured(&file.settings),
        name: file.name,
        revision,
    })
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
