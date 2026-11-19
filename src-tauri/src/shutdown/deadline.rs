use anyhow::{Context, Result, ensure};
use std::{
    io::{self, BufRead, Read, Write},
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    process::{Child, Command, Stdio},
    ptr::{null, null_mut},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, DuplicateHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        CREATE_NO_WINDOW, CreateEventW, CreateWaitableTimerW, GetCurrentProcess,
        GetCurrentProcessId, GetProcessId, INFINITE, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_TERMINATE, SYNCHRONIZATION_SYNCHRONIZE, SetEvent, SetWaitableTimer,
        TerminateProcess, WaitForMultipleObjects, WaitForSingleObject,
    },
};

pub(crate) const FLAG: &str = "--exit-deadline";
pub(crate) const TIMEOUT_EXIT_CODE: u32 = 124;

pub(crate) struct Deadline {
    timer: OwnedHandle,
    worker: Child,
}

impl Deadline {
    pub(crate) fn launch(mut command: Command) -> Result<Self> {
        let timer = owned(
            unsafe { CreateWaitableTimerW(null(), 1, null()) },
            "创建退出计时器",
        )?;
        let ready = owned(
            unsafe { CreateEventW(null(), 1, 0, null()) },
            "创建监护就绪事件",
        )?;
        let mut child = command
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .context("启动退出监护进程")?;
        if let Err(error) = connect(&mut child, &timer, &ready) {
            let _ = child.kill();
            return Err(error);
        }
        Ok(Self {
            timer,
            worker: child,
        })
    }

    pub(crate) fn arm(&self, budget: Duration) -> Result<()> {
        ensure!(
            unsafe { WaitForSingleObject(self.worker.as_raw_handle(), 0) } == WAIT_TIMEOUT,
            "退出监护已经结束"
        );
        let due = -i64::try_from(budget.as_nanos() / 100)?.max(1);
        if unsafe { SetWaitableTimer(self.timer.as_raw_handle(), &due, 0, None, null(), 0) } == 0 {
            return Err(io::Error::last_os_error()).context("设置退出期限");
        }
        Ok(())
    }
}

fn connect(child: &mut Child, timer: &OwnedHandle, ready: &OwnedHandle) -> Result<()> {
    let remote = child.as_raw_handle();
    let parent = duplicate(
        remote,
        unsafe { GetCurrentProcess() },
        SYNCHRONIZATION_SYNCHRONIZE | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
    )?;
    let clock = duplicate(remote, timer.as_raw_handle(), SYNCHRONIZATION_SYNCHRONIZE)?;
    let signal = duplicate(
        remote,
        ready.as_raw_handle(),
        windows_sys::Win32::System::Threading::EVENT_MODIFY_STATE,
    )?;
    let mut input = child.stdin.take().context("退出监护输入管道不可用")?;
    writeln!(input, "{parent} {clock} {signal}").context("交接退出监护句柄")?;
    drop(input);
    let wait = [ready.as_raw_handle(), remote];
    let status = unsafe { WaitForMultipleObjects(wait.len() as u32, wait.as_ptr(), 0, 3000) };
    ensure!(
        status == WAIT_OBJECT_0,
        "退出监护未就绪，Windows 等待状态 {status}"
    );
    Ok(())
}

fn duplicate(destination: HANDLE, source: HANDLE, access: u32) -> Result<usize> {
    let mut handle = null_mut();
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            source,
            destination,
            &mut handle,
            access,
            0,
            0,
        )
    } == 0
    {
        return Err(io::Error::last_os_error()).context("复制退出监护句柄");
    }
    Ok(handle as usize)
}

fn owned(handle: HANDLE, operation: &'static str) -> Result<OwnedHandle> {
    if handle.is_null() {
        return Err(io::Error::last_os_error()).context(operation);
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

pub(crate) fn run_child() -> Result<()> {
    let [parent, timer, ready] = read_handles()?;
    let id = unsafe { GetProcessId(parent) };
    ensure!(
        id != 0 && id != unsafe { GetCurrentProcessId() },
        "退出监护主进程句柄无效"
    );
    ensure!(
        unsafe { WaitForSingleObject(timer, 0) } == WAIT_TIMEOUT,
        "退出计时器句柄无效"
    );
    if unsafe { SetEvent(ready) } == 0 {
        return Err(io::Error::last_os_error()).context("确认退出监护就绪");
    }
    unsafe { CloseHandle(ready) };

    // 两个句柄均由原进程直接复制给本进程。监护从不按 PID 打开目标。
    let wait = [parent, timer];
    match unsafe { WaitForMultipleObjects(wait.len() as u32, wait.as_ptr(), 0, INFINITE) } {
        WAIT_OBJECT_0 => Ok(()),
        status if status == WAIT_OBJECT_0 + 1 => {
            if unsafe { TerminateProcess(parent, TIMEOUT_EXIT_CODE) } == 0 {
                let error = io::Error::last_os_error();
                if unsafe { WaitForSingleObject(parent, 0) } != WAIT_OBJECT_0 {
                    return Err(error).context("终止超时的控制台主进程");
                }
            }
            Ok(())
        }
        status => anyhow::bail!("退出监护无法继续等待，Windows 状态 {status}"),
    }
}

fn read_handles() -> Result<[HANDLE; 3]> {
    let mut line = String::new();
    io::BufReader::new(io::stdin().lock().take(128)).read_line(&mut line)?;
    ensure!(line.ends_with('\n'), "退出监护句柄帧未完成");
    let mut words = line.split_ascii_whitespace();
    let mut result = [null_mut(); 3];
    for handle in &mut result {
        let raw: usize = words.next().context("退出监护缺少句柄")?.parse()?;
        ensure!(raw != 0 && raw <= isize::MAX as usize, "退出监护句柄无效");
        *handle = raw as HANDLE;
    }
    ensure!(words.next().is_none(), "退出监护存在额外参数");
    Ok(result)
}

pub(crate) fn arm_or_terminate(deadline: Option<&Deadline>, budget: Duration) {
    if deadline.is_none_or(|deadline| deadline.arm(budget).is_err()) {
        unsafe { TerminateProcess(GetCurrentProcess(), TIMEOUT_EXIT_CODE) };
        std::process::abort();
    }
}
