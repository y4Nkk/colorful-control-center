use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows_sys::Win32::{
    Foundation::*,
    System::{Com::CoCreateGuid, Threading::*},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub parent_id: u32,
    pub token: String,
}
impl Binding {
    pub fn validate(&self) -> Result<()> {
        if self.parent_id == 0
            || self.token.len() != 32
            || !self.token.bytes().all(|c| c.is_ascii_hexdigit())
        {
            bail!("系统授权会话绑定无效");
        }
        Ok(())
    }
    pub(crate) fn new() -> Result<Self> {
        let mut guid = windows_sys::core::GUID::default();
        let code = unsafe { CoCreateGuid(&mut guid) };
        if code < 0 {
            bail!("无法创建系统会话标识：{code}");
        }
        Ok(Self {
            parent_id: std::process::id(),
            token: format!(
                "{:08x}{:04x}{:04x}{}",
                guid.data1,
                guid.data2,
                guid.data3,
                guid.data4
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ),
        })
    }
    fn name(&self, role: &str) -> Vec<u16> {
        format!(
            "Local\\{}.authorization.{}.{}.{}",
            env!("CARGO_PKG_NAME"),
            self.parent_id,
            self.token,
            role
        )
        .encode_utf16()
        .chain([0])
        .collect()
    }
}
pub(crate) struct Event(OwnedHandle);
impl Event {
    fn create(binding: &Binding, role: &str) -> Result<Self> {
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, binding.name(role).as_ptr()) };
        let error = unsafe { GetLastError() };
        let event = Self::owned(handle)?;
        if error == ERROR_ALREADY_EXISTS {
            bail!("系统会话对象已存在，未复用旧许可");
        }
        Ok(event)
    }
    fn open(binding: &Binding, role: &str) -> Result<Self> {
        Self::owned(unsafe {
            OpenEventW(
                EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE,
                0,
                binding.name(role).as_ptr(),
            )
        })
    }
    fn owned(handle: HANDLE) -> Result<Self> {
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(handle) }))
    }
    pub fn parent(id: u32) -> Result<Self> {
        Self::owned(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, id) })
    }
    pub fn raw(&self) -> HANDLE {
        self.0.as_raw_handle()
    }
    pub fn signal(&self) -> Result<()> {
        if unsafe { SetEvent(self.raw()) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }
    pub fn reset(&self) -> Result<()> {
        if unsafe { ResetEvent(self.raw()) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }
    pub fn signaled(&self) -> Result<bool> {
        match unsafe { WaitForSingleObject(self.raw(), 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(std::io::Error::last_os_error().into()),
        }
    }
}
pub(crate) struct Events {
    pub ready: Event,
    pub execute: Event,
    pub rollback: Event,
    pub finish: Event,
    pub cancel: Event,
    pub response: Event,
}
impl Events {
    pub fn create(binding: &Binding) -> Result<Self> {
        Self::build(binding, Event::create)
    }
    pub fn open(binding: &Binding) -> Result<Self> {
        Self::build(binding, Event::open)
    }
    fn build(binding: &Binding, make: fn(&Binding, &str) -> Result<Event>) -> Result<Self> {
        binding.validate()?;
        Ok(Self {
            ready: make(binding, "ready")?,
            execute: make(binding, "execute")?,
            rollback: make(binding, "rollback")?,
            finish: make(binding, "finish")?,
            cancel: make(binding, "cancel")?,
            response: make(binding, "response")?,
        })
    }
}
