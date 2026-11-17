//! 主控制者与监护共同保留对象句柄；最后一个句柄关闭前不能重新接管。
use anyhow::{Result, anyhow, bail};
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
    System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE},
};

pub const NAME: &str = concat!("Global\\", env!("CARGO_PKG_NAME"), ".controller");

pub struct ControllerLease {
    _handle: OwnedHandle,
}

fn wide(name: &str) -> Result<Vec<u16>> {
    if name.is_empty() || name.contains('\0') {
        bail!("控制权对象名称无效");
    }
    Ok(name.encode_utf16().chain([0]).collect())
}

impl ControllerLease {
    pub fn try_acquire(name: &str) -> Result<Option<Self>> {
        let name = wide(name)?;
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        let error = unsafe { GetLastError() };
        if handle.is_null() {
            bail!("建立控制权失败：Windows {error}");
        }
        let lease = Self {
            _handle: unsafe { OwnedHandle::from_raw_handle(handle) },
        };
        if error == ERROR_ALREADY_EXISTS {
            return Ok(None);
        }
        Ok(Some(lease))
    }

    pub fn retain(name: &str) -> Result<Self> {
        let name = wide(name)?;
        let handle = unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(anyhow!(
                "主控制权不存在或不可访问，监护未启用：Windows {}",
                unsafe { GetLastError() }
            ));
        }
        Ok(Self {
            _handle: unsafe { OwnedHandle::from_raw_handle(handle) },
        })
    }
}
