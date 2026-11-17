use super::*;
use std::cell::RefCell;

struct State {
    page: [u8; 256],
    reads: usize,
    writes: Vec<u8>,
    fail_reads: Vec<usize>,
    fail_writes: Vec<usize>,
    ignore_writes: Vec<usize>,
    device_commands: Vec<i32>,
    invalid_ranges: usize,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        page: [0xA5; 256], reads: 0, writes: vec![], fail_reads: vec![],
        fail_writes: vec![], ignore_writes: vec![], device_commands: vec![], invalid_ranges: 0,
    });
}
fn reset(value: u8) -> Dchu {
    STATE.with_borrow_mut(|state| {
        state.page = [0xA5; 256];
        state.page[6] = value;
        state.reads = 0;
        state.writes.clear();
        state.fail_reads.clear();
        state.fail_writes.clear();
        state.ignore_writes.clear();
        state.device_commands.clear();
        state.invalid_ranges = 0;
    });
    Dchu::test_io(get_buffer, read, send, write)
}
unsafe extern "system" fn get_buffer(command: i32, _: *mut u8) -> i32 {
    STATE.with_borrow_mut(|state| state.device_commands.push(command));
    0
}
unsafe extern "system" fn send(command: i32, _: *const u8, _: i32) -> i32 {
    STATE.with_borrow_mut(|state| state.device_commands.push(command));
    0
}
unsafe extern "system" fn read(page: i32, offset: i32, length: i32, buffer: *mut u8) -> i32 {
    STATE.with_borrow_mut(|state| {
        state.reads += 1;
        if (page, offset, length) != (1, 6, 1) {
            state.invalid_ranges += 1;
            return 0;
        }
        if state.fail_reads.contains(&state.reads) {
            return 0;
        }
        unsafe {
            buffer.write(state.page[6]);
        }
        4096
    })
}
unsafe extern "system" fn write(page: i32, offset: i32, length: i32, buffer: *const u8) -> i32 {
    STATE.with_borrow_mut(|state| {
        if (page, offset, length) != (1, 6, 1) {
            state.invalid_ranges += 1;
            return 0;
        }
        let value = unsafe { buffer.read() };
        state.writes.push(value);
        let attempt = state.writes.len();
        if !state.ignore_writes.contains(&attempt) {
            state.page[6] = value;
        }
        if state.fail_writes.contains(&attempt) {
            0
        } else {
            1
        }
    })
}
fn assert_isolated() {
    STATE.with_borrow(|state| {
        assert!(state.device_commands.is_empty());
        assert_eq!(state.invalid_ranges, 0);
        for (offset, value) in state.page.iter().enumerate() {
            if offset != 6 {
                assert_eq!(*value, 0xA5);
            }
        }
    });
}

#[test]
fn all_reports_sync_only_the_original_camera_byte_without_device_commands() {
    for (report, target) in [
        (CameraReport::Camera(false), 0),
        (CameraReport::Camera(true), 1),
        (CameraReport::CameraMicrophone(true), 1),
    ] {
        let hal = Hal::test_instance(reset(0xFE), None);
        let permission = identity(&hal).unwrap();
        STATE.with_borrow(|state| assert!(state.writes.is_empty()));
        let result = handle(&hal, &permission, report).unwrap();
        assert!(result.detail.contains("原厂报告"));
        assert!(result.detail.contains("保存值已同步"));
        assert!(!result.error);
        assert!(result.level.is_none());
        STATE.with_borrow(|state| {
            assert_eq!(state.page[6], target);
            assert_eq!(state.writes, [target]);
        });
        handle(&hal, &permission, report).unwrap();
        STATE.with_borrow(|state| assert_eq!(state.writes, [target]));
        assert_isolated();
    }
}

#[test]
fn combined_off_is_only_a_report_and_does_not_read_or_write_saved_camera() {
    let io = reset(1);
    STATE.with_borrow_mut(|state| state.fail_reads.push(1));
    let report = CameraReport::CameraMicrophone(false);
    synchronize(&io, report, IDENTITY).unwrap();
    STATE.with_borrow(|state| {
        assert_eq!(state.reads, 0);
        assert!(state.writes.is_empty());
        assert_eq!(state.page[6], 1);
    });
    assert_eq!(message(report).detail, "原厂报告：已关闭");
    assert_eq!(message(report).title, "摄像头与麦克风");
    assert_isolated();
}

#[test]
fn stale_permission_and_failed_initial_read_never_write() {
    let io = reset(1);
    assert!(synchronize(&io, CameraReport::Camera(false), "old-rules").is_err());
    STATE.with_borrow(|state| assert_eq!(state.reads, 0));
    STATE.with_borrow_mut(|state| state.fail_reads.push(1));
    assert!(synchronize(&io, CameraReport::Camera(false), IDENTITY).is_err());
    STATE.with_borrow(|state| assert!(state.writes.is_empty()));
    assert_isolated();
}

#[test]
fn partial_save_ignored_save_and_readback_failures_restore_exact_unknown_previous_value() {
    for failure in 0..3 {
        let io = reset(0xFE);
        STATE.with_borrow_mut(|state| match failure {
            0 => state.fail_writes.push(1),
            1 => state.ignore_writes.push(1),
            _ => state.fail_reads.push(2),
        });
        let error = synchronize(&io, CameraReport::Camera(true), IDENTITY).unwrap_err();
        assert!(format!("{error:#}").contains("原保存值已恢复"));
        STATE.with_borrow(|state| {
            assert_eq!(state.page[6], 0xFE);
            assert_eq!(state.writes, [1, 0xFE]);
        });
        assert_isolated();
    }
}

#[test]
fn compensation_failures_keep_both_diagnostics_and_a_later_report_can_resynchronize() {
    for fail_restore_read in [false, true] {
        let io = reset(0xFE);
        STATE.with_borrow_mut(|state| {
            state.fail_writes.push(1);
            if fail_restore_read {
                state.fail_reads.push(2);
            } else {
                state.fail_writes.push(2);
            }
        });
        let error = synchronize(&io, CameraReport::Camera(true), IDENTITY).unwrap_err();
        let diagnostic = format!("{error:#}");
        assert!(diagnostic.contains("同步失败"));
        assert!(diagnostic.contains("原保存值恢复失败"));
        assert!(diagnostic.contains("WriteAppSettings"));
        STATE.with_borrow_mut(|state| {
            state.fail_writes.clear();
            state.fail_reads.clear();
        });
        synchronize(&io, CameraReport::Camera(true), IDENTITY).unwrap();
        STATE.with_borrow(|state| assert_eq!(state.page[6], 1));
        assert_isolated();
    }
}

#[test]
fn capability_failure_and_invalid_runtime_permission_do_not_mutate_the_cache() {
    let hal = Hal::test_instance(reset(0), None);
    STATE.with_borrow_mut(|state| state.fail_reads.push(1));
    assert!(identity(&hal).unwrap_err().to_string().contains("不可读取"));
    assert!(handle(&hal, "old-rules", CameraReport::Camera(true)).is_err());
    STATE.with_borrow(|state| {
        assert_eq!(state.reads, 1);
        assert!(state.writes.is_empty());
    });
    assert_isolated();
}
