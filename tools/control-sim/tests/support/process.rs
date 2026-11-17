use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, SYNCHRONIZATION_SYNCHRONIZE,
        WaitForSingleObject,
    },
};

/// 仅查询本次 Fixture 记录的子进程 PID，不结束其他进程。
pub fn alive(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZATION_SYNCHRONIZE,
            0,
            pid,
        );
        if handle.is_null() {
            assert_eq!(GetLastError(), 87, "无法查询本次隔离子进程");
            return false;
        }
        // ExitProcess已公布退出码时，进程内核对象仍可能尚未完成退出。
        let status = WaitForSingleObject(handle, 0);
        CloseHandle(handle);
        assert!(status == WAIT_OBJECT_0 || status == WAIT_TIMEOUT);
        status == WAIT_TIMEOUT
    }
}
