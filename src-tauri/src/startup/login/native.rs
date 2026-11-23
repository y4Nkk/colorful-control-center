use super::{
    model::{self, Record, State},
    transaction::Io,
};
use anyhow::{Result, bail};
use std::path::PathBuf;
use windows::{
    Win32::System::{Com::*, TaskScheduler::*, Variant::VARIANT},
    core::BSTR,
};
use crate::hal::authorization;
use windows_sys::Win32::Security;

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
pub struct Native {
    folder: ITaskFolder,
    _apartment: Apartment,
    pub user: String,
    pub user_name: String,
    pub executable: String,
    pub path: PathBuf,
}
// 任务服务存储任务时会把 LogonTrigger 的 SID 规范化成 机器\用户，指纹需要第二形态。
pub fn user_name() -> Result<String> {
    let mut buffer = [0u16; 512];
    let mut size = buffer.len() as u32;
    if !unsafe {
        Security::Authentication::Identity::GetUserNameExW(
            Security::Authentication::Identity::NameSamCompatible,
            buffer.as_mut_ptr(),
            &mut size,
        )
    } {
        return Err(std::io::Error::last_os_error().into());
    }
    let end = buffer
        .iter()
        .position(|&value| value == 0)
        .unwrap_or(size as usize);
    Ok(String::from_utf16_lossy(&buffer[..end]))
}
impl Native {
    pub fn new(path: PathBuf) -> Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
        let apartment = Apartment;
        let service: ITaskService =
            unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)? };
        let empty = VARIANT::default();
        unsafe { service.Connect(&empty, &empty, &empty, &empty)? };
        Ok(Self {
            folder: unsafe { service.GetFolder(&BSTR::from("\\"))? },
            _apartment: apartment,
            user: authorization::user()?,
            user_name: user_name()?,
            executable: std::env::current_exe()?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("程序路径不是UTF-8"))?
                .into(),
            path,
        })
    }
    fn name(&self) -> BSTR {
        BSTR::from(format!("ColorfulControlCenter-{}", self.user))
    }
    fn raw(&self) -> Result<Option<String>> {
        match unsafe { self.folder.GetTask(&self.name()) } {
            Ok(task) => Ok(Some(unsafe { task.Xml()? }.to_string())),
            Err(error) if error.code().0 as u32 == 0x80070002 => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}
impl Io for Native {
    fn state(&self) -> Result<State> {
        let raw = self.raw()?;
        let task = raw
            .as_ref()
            .map(|value| {
                model::task_level(value, &self.executable, &self.user, &self.user_name)
                    .map(|level| model::xml(&self.executable, &self.user, level).unwrap())
            })
            .transpose()?;
        let state = State {
            task,
            run: super::super::registry::read_value()?,
        };
        model::validate(&state, &self.executable, &self.user, &self.user_name)?;
        Ok(state)
    }
    fn record(&self) -> Result<Option<Record>> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if bytes.len() > 262144 {
            bail!("登录自启恢复记录过大");
        }
        let Some(record): Option<Record> = serde_json::from_slice(&bytes)? else {
            return Ok(None);
        };
        if record.version != 1 {
            bail!("登录自启恢复记录版本无效");
        }
        model::validate(&record.before, &self.executable, &self.user, &self.user_name)?;
        model::validate(&record.applied, &self.executable, &self.user, &self.user_name)?;
        Ok(Some(record))
    }
    fn task(&self, xml: Option<&str>) -> Result<()> {
        if let Some(xml) = xml {
            model::task_level(xml, &self.executable, &self.user, &self.user_name)?;
            let empty = VARIANT::default();
            unsafe {
                self.folder.RegisterTask(
                    &self.name(),
                    &BSTR::from(xml),
                    TASK_CREATE_OR_UPDATE.0,
                    &VARIANT::from(self.user.as_str()),
                    &empty,
                    TASK_LOGON_INTERACTIVE_TOKEN,
                    &empty,
                )?
            };
        } else if self.raw()?.is_some() {
            unsafe { self.folder.DeleteTask(&self.name(), 0)? };
        }
        Ok(())
    }
    fn run(&self, value: Option<&str>) -> Result<()> {
        super::super::registry::write_value(value)
    }
    fn save(&self, record: &Option<Record>) -> Result<()> {
        std::fs::create_dir_all(
            self.path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("恢复记录没有目录"))?,
        )?;
        crate::storage::write(&self.path, &serde_json::to_vec(record)?)?;
        Ok(())
    }
}
