use super::Capacity;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    io::{Error, Result},
    time::{Duration, Instant},
};
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    GetVolumeNameForVolumeMountPointW,
};

const METADATA_MAX_AGE: Duration = Duration::from_secs(120);
const DRIVE_FIXED: u32 = 3;
type VolumeGuid = [u16; 50];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub mount: String,
    pub label: Option<String>,
    pub file_system: Option<String>,
    pub capacity: Option<Capacity>,
}

struct Metadata {
    guid: VolumeGuid,
    read_at: Instant,
    label: String,
    file_system: String,
}

pub(super) struct MetadataCache(BTreeMap<u8, Metadata>);

impl MetadataCache {
    pub(super) const fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub(super) fn read(
        &mut self,
        refresh: bool,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<Volume>> {
        let logical_drives = unsafe { GetLogicalDrives() };
        if logical_drives == 0 {
            let error = Error::last_os_error();
            self.0.clear();
            return Err(error);
        }
        if refresh {
            self.0.clear();
        }
        let mut fixed_drives = 0;
        let volumes = (0..26)
            .filter(|index| logical_drives & (1 << index) != 0)
            .filter_map(|index| {
                let letter = b'A' + index as u8;
                let root = [u16::from(letter), b':' as u16, b'\\' as u16, 0];
                if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
                    return None;
                }
                fixed_drives |= 1 << index;
                Some(self.read_volume(letter, &root, warnings))
            })
            .collect();
        self.0
            .retain(|letter, _| fixed_drives & (1 << (*letter - b'A')) != 0);
        Ok(volumes)
    }

    fn read_volume(&mut self, letter: u8, root: &[u16], warnings: &mut Vec<String>) -> Volume {
        let mount = format!("{}:", char::from(letter));
        let guid = volume_guid(root);
        let path = guid.as_ref().map_or(root, |guid| guid.as_slice());
        let capacity = capacity(path)
            .map_err(|error| {
                warnings.push(format!("{mount} 容量读取失败：{error}"));
            })
            .ok();
        let (label, file_system) =
            match self.metadata(letter, guid, Instant::now(), || read_metadata(path)) {
                Ok((label, file_system)) => (Some(label), Some(file_system)),
                Err(error) => {
                    warnings.push(format!("{mount} 卷信息读取失败：{error}"));
                    (None, None)
                }
            };
        Volume {
            mount,
            label,
            file_system,
            capacity,
        }
    }

    fn metadata(
        &mut self,
        letter: u8,
        guid: Option<VolumeGuid>,
        now: Instant,
        read: impl FnOnce() -> Result<(String, String)>,
    ) -> Result<(String, String)> {
        if let Some(cached) = self.0.get(&letter)
            && Some(cached.guid) == guid
            && now.duration_since(cached.read_at) < METADATA_MAX_AGE
        {
            return Ok((cached.label.clone(), cached.file_system.clone()));
        }
        self.0.remove(&letter);
        let (label, file_system) = read()?;
        if let Some(guid) = guid {
            self.0.insert(
                letter,
                Metadata {
                    guid,
                    read_at: now,
                    label: label.clone(),
                    file_system: file_system.clone(),
                },
            );
        }
        Ok((label, file_system))
    }
}

fn volume_guid(root: &[u16]) -> Option<VolumeGuid> {
    let mut guid = [0; 50];
    let ok = unsafe {
        GetVolumeNameForVolumeMountPointW(root.as_ptr(), guid.as_mut_ptr(), guid.len() as u32)
    };
    (ok != 0).then_some(guid)
}

fn capacity(path: &[u16]) -> Result<Capacity> {
    let (mut available_bytes, mut total_bytes) = (0, 0);
    if unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available_bytes,
            &mut total_bytes,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(Error::last_os_error());
    }
    Ok(Capacity {
        total_bytes,
        available_bytes,
    })
}

fn read_metadata(path: &[u16]) -> Result<(String, String)> {
    let mut label = [0u16; 261];
    let mut file_system = [0u16; 32];
    if unsafe {
        GetVolumeInformationW(
            path.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            file_system.as_mut_ptr(),
            file_system.len() as u32,
        )
    } == 0
    {
        return Err(Error::last_os_error());
    }
    let text = |buffer: &[u16]| {
        let length = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..length])
    };
    Ok((text(&label), text(&file_system)))
}

#[cfg(test)]
mod tests;
