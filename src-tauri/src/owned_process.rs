use anyhow::{Context, Result, bail};
use std::{
    ffi::{OsStr, OsString},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
};
use windows_sys::Win32::{
    Foundation::*,
    System::{JobObjects::*, Threading::*},
};

fn quoted(value: &OsStr) -> Result<Vec<u16>> {
    let mut output = vec![34];
    let mut slashes = 0;
    for character in value.encode_wide() {
        if character == 0 {
            bail!("子进程参数含空字符");
        }
        if character == 92 {
            slashes += 1;
            continue;
        }
        output.extend(std::iter::repeat_n(
            92,
            slashes * if character == 34 { 2 } else { 1 },
        ));
        slashes = 0;
        if character == 34 {
            output.push(92);
        }
        output.push(character);
    }
    output.extend(std::iter::repeat_n(92, slashes * 2));
    output.push(34);
    Ok(output)
}
fn command_line(executable: &Path, arguments: &[OsString]) -> Result<Vec<u16>> {
    let mut command = quoted(executable.as_os_str())?;
    for argument in arguments {
        command.push(32);
        command.extend(quoted(argument)?);
    }
    if command.len() >= 32767 {
        bail!("子进程命令行超出Windows长度范围");
    }
    command.push(0);
    Ok(command)
}
struct Attributes {
    _storage: Vec<usize>,
    _jobs: Box<[HANDLE; 1]>,
    pointer: LPPROC_THREAD_ATTRIBUTE_LIST,
}
impl Attributes {
    fn new(job: HANDLE) -> Result<Self> {
        let mut size = 0;
        let first =
            unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size) };
        let error = std::io::Error::last_os_error();
        if first != 0
            || error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32)
            || size == 0
            || size > 65536
        {
            bail!("无法确定子进程属性长度：{error}");
        }
        let mut storage = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
        let pointer = storage.as_mut_ptr().cast();
        if unsafe { InitializeProcThreadAttributeList(pointer, 1, 0, &mut size) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut attributes = Self {
            _storage: storage,
            _jobs: Box::new([job]),
            pointer,
        };
        if unsafe {
            UpdateProcThreadAttribute(
                attributes.pointer,
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                attributes._jobs.as_mut_ptr().cast(),
                std::mem::size_of::<HANDLE>(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(attributes)
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.pointer);
        }
    }
}

pub struct Process {
    process: OwnedHandle,
    job: OwnedHandle,
    thread: Option<OwnedHandle>,
    id: u32,
}
impl Process {
    pub fn prepare(executable: &Path, arguments: &[OsString]) -> Result<Self> {
        if !executable.is_absolute() {
            bail!("子进程路径必须是明确的绝对路径");
        }
        let mut application = executable.as_os_str().encode_wide().collect::<Vec<_>>();
        if application.contains(&0) {
            bail!("子进程路径含空字符");
        }
        application.push(0);
        let mut command = command_line(executable, arguments)?;
        let raw_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw_job.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        let job = unsafe { OwnedHandle::from_raw_handle(raw_job) };
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                raw_job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let attributes = Attributes::new(raw_job)?;
        let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
        startup.StartupInfo.cb = std::mem::size_of_val(&startup) as u32;
        startup.lpAttributeList = attributes.pointer;
        let mut information: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe {
            CreateProcessW(
                application.as_ptr(),
                command.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                CREATE_SUSPENDED | CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT,
                std::ptr::null(),
                std::ptr::null(),
                &startup.StartupInfo,
                &mut information,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error()).context("创建受管子进程失败");
        }
        Ok(Self {
            process: unsafe { OwnedHandle::from_raw_handle(information.hProcess) },
            job,
            thread: Some(unsafe { OwnedHandle::from_raw_handle(information.hThread) }),
            id: information.dwProcessId,
        })
    }
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn start(&mut self) -> Result<()> {
        let thread = self
            .thread
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("子进程已经启动"))?;
        if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
            return Err(std::io::Error::last_os_error()).context("启动受管子进程失败");
        }
        self.thread.take();
        Ok(())
    }
    pub fn poll(&self) -> Result<Option<u32>> {
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) } == 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                Ok(Some(code))
            }
            _ => Err(std::io::Error::last_os_error()).context("核对受管子进程状态失败"),
        }
    }
    pub fn stop(&mut self) -> Result<()> {
        if unsafe { TerminateJobObject(self.job.as_raw_handle(), 0) } == 0 {
            return Err(std::io::Error::last_os_error()).context("结束受管子进程失败");
        }
        if unsafe { WaitForSingleObject(self.process.as_raw_handle(), 3000) } != WAIT_OBJECT_0 {
            bail!("受管子进程未在期限内结束");
        }
        Ok(())
    }
}
