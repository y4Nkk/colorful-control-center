use super::super::{
    model::{Saver, Source},
    resource::{Event, Input, Resource},
};
use crate::{hal::key_events, owned_process::Process};
use anyhow::{Result, bail};
use ccc_core::panel_hotkey::{Activity, Kind};
use sha2::{Digest, Sha256};
use std::{ffi::OsString, io::Read, os::windows::ffi::OsStringExt, path::PathBuf, sync::mpsc};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

pub(super) fn saver() -> Result<Saver> {
    let mut directory = [0u16; 32768];
    let length =
        unsafe { GetSystemDirectoryW(directory.as_mut_ptr(), directory.len() as u32) } as usize;
    if length == 0 || length >= directory.len() {
        bail!("无法确定Windows系统目录");
    }
    let path = PathBuf::from(OsString::from_wide(&directory[..length])).join("scrnsave.scr");
    let mut bytes = Vec::new();
    std::fs::File::open(&path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
        bail!("Windows空白屏保文件长度无效");
    }
    Ok(Saver {
        path,
        fingerprint: format!("{:x}", Sha256::digest(&bytes)),
    })
}
pub(super) fn prepare(source: &Source) -> Result<Box<dyn Resource>> {
    match source.kind {
        Kind::BlankSaver => {
            let saver = source
                .saver
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("空白屏保来源缺失"))?;
            let process = Process::prepare(&saver.path, &["/s".into()])?;
            log::debug!("[panel-hotkey] prepared blank saver pid={}", process.id());
            Ok(Box::new(Blank(process)))
        }
        Kind::PointerWake => Ok(Box::new(Pointer::prepare()?)),
    }
}
struct Blank(Process);
impl Resource for Blank {
    fn start(&mut self) -> Result<()> {
        self.0.start()
    }
    fn poll(&mut self) -> Result<Event> {
        Ok(self.0.poll()?.map(Event::Exited).unwrap_or(Event::Pending))
    }
    fn stop(&mut self) -> Result<()> {
        self.0.stop()
    }
}
struct Pointer {
    input: Input,
    receiver: mpsc::Receiver<Activity>,
    session: Option<key_events::Session>,
}
impl Pointer {
    fn prepare() -> Result<Self> {
        let (input, receiver) = Input::channel();
        let events = input.clone();
        let session =
            key_events::Session::input_activity(move |activity| events.submit(activity), |_| {})?;
        Ok(Self {
            input,
            receiver,
            session: Some(session),
        })
    }
    fn check(&self) -> Result<()> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("输入唤醒监听已结束"))?;
        if let Some(error) = session.error() {
            bail!("输入唤醒监听失败：{error}");
        }
        Ok(())
    }
}
impl Resource for Pointer {
    fn start(&mut self) -> Result<()> {
        self.check()?;
        self.input.activate();
        Ok(())
    }
    fn poll(&mut self) -> Result<Event> {
        self.check()?;
        match self.receiver.try_recv() {
            Ok(activity) => Ok(Event::Input(activity)),
            Err(mpsc::TryRecvError::Empty) => Ok(Event::Pending),
            Err(mpsc::TryRecvError::Disconnected) => bail!("输入唤醒监听通道已断开"),
        }
    }
    fn stop(&mut self) -> Result<()> {
        self.input.stop();
        self.session.take();
        Ok(())
    }
}
impl Drop for Pointer {
    fn drop(&mut self) {
        self.input.stop();
        self.session.take();
    }
}
