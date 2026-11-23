use crate::hal::authorization::{current_token, elevated};
use anyhow::{Context, Result};
use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
};
use windows_sys::Win32::{
    Security::*,
    System::{SystemServices::SE_GROUP_INTEGRITY, Threading::SetThreadToken},
};

pub(super) fn create_with_window_integrity<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    if !elevated()? {
        return operation();
    }
    let medium = integrity_token(WinMediumLabelSid)?;
    with_thread_token(&medium, operation)
}

fn integrity_token(level: WELL_KNOWN_SID_TYPE) -> Result<OwnedHandle> {
    let current = current_token(TOKEN_DUPLICATE)?;
    let mut duplicate = std::ptr::null_mut();
    if unsafe {
        DuplicateTokenEx(
            current.as_raw_handle(),
            TOKEN_ADJUST_DEFAULT | TOKEN_IMPERSONATE,
            std::ptr::null(),
            SecurityImpersonation,
            TokenImpersonation,
            &mut duplicate,
        )
    } == 0
    {
        return Err(io::Error::last_os_error()).context("复制当前账户的线程令牌");
    }
    let duplicate = unsafe { OwnedHandle::from_raw_handle(duplicate) };
    let mut buffer = [0u32; SECURITY_MAX_SID_SIZE as usize / size_of::<u32>()];
    let sid = buffer.as_mut_ptr().cast();
    let mut bytes = size_of_val(&buffer) as u32;
    if unsafe { CreateWellKnownSid(level, std::ptr::null_mut(), sid, &mut bytes) } == 0 {
        return Err(io::Error::last_os_error()).context("生成唤回事件的完整性标识");
    }
    let label = TOKEN_MANDATORY_LABEL {
        Label: SID_AND_ATTRIBUTES {
            Sid: sid,
            Attributes: SE_GROUP_INTEGRITY as u32,
        },
    };
    if unsafe {
        SetTokenInformation(
            duplicate.as_raw_handle(),
            TokenIntegrityLevel,
            (&label as *const TOKEN_MANDATORY_LABEL).cast(),
            size_of_val(&label) as u32 + bytes,
        )
    } == 0
    {
        return Err(io::Error::last_os_error()).context("降低唤回事件创建令牌的完整性级别");
    }
    Ok(duplicate)
}

fn with_thread_token<T>(token: &OwnedHandle, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    if unsafe { SetThreadToken(std::ptr::null(), token.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error()).context("设置唤回事件的线程令牌");
    }
    struct RevertOnUnwind;
    impl Drop for RevertOnUnwind {
        fn drop(&mut self) {
            unsafe { RevertToSelf() };
        }
    }
    let guard = RevertOnUnwind;
    let result = operation();
    let reverted = unsafe { RevertToSelf() };
    std::mem::forget(guard);
    if reverted == 0 {
        return Err(io::Error::last_os_error()).context("恢复应用的线程令牌失败");
    }
    result
}

#[cfg(test)]
mod integrity_tests {
    use super::*;
    use crate::hal::authorization::session::events::{Binding, Event};
    use windows_sys::Win32::{
        Foundation::{ERROR_NO_TOKEN, GetLastError},
        System::Threading::{
            CreateEventW, EVENT_MODIFY_STATE, GetCurrentThread, OpenEventW, OpenThreadToken,
        },
    };

    #[test]
    fn activation_object_inherits_the_temporary_threads_integrity() {
        let low = integrity_token(WinLowLabelSid).unwrap();
        let name: Vec<u16> = format!(
            "Local\\CCC.test-integrity.{}",
            Binding::new().unwrap().token
        )
        .encode_utf16()
        .chain([0])
        .collect();
        let event = with_thread_token(&low, || {
            Event::owned(unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) })
        })
        .unwrap();
        with_thread_token(&low, || {
            let writer = Event::owned(unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) })?;
            writer.signal()
        })
        .unwrap();
        assert!(event.signaled().unwrap());
        let mut effective = std::ptr::null_mut();
        assert_eq!(
            unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut effective) },
            0
        );
        assert_eq!(unsafe { GetLastError() }, ERROR_NO_TOKEN);
    }
}
