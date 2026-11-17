use super::*;
use std::sync::Mutex;

struct Record {
    commands: Vec<(i32, Vec<u8>)>,
    fail_release: bool,
    fail_read: bool,
}
static RECORD: Mutex<Record> = Mutex::new(Record {
    commands: Vec::new(),
    fail_release: false,
    fail_read: false,
});

unsafe extern "system" fn package(_: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn read(_: i32, _: i32, _: i32, output: *mut u8) -> i32 {
    if RECORD.lock().unwrap().fail_read {
        return 0;
    }
    unsafe {
        output.write(0);
    }
    4096
}
unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    RECORD.lock().unwrap().commands.push((-1, vec![0]));
    1
}
unsafe extern "system" fn set(command: i32, bytes: *const u8, length: i32) -> i32 {
    let mut record = RECORD.lock().unwrap();
    record.commands.push((
        command,
        unsafe { std::slice::from_raw_parts(bytes, length as usize) }.to_vec(),
    ));
    if command == 0x69 && record.fail_release {
        return 0;
    }
    command
}

#[test]
fn child_restore_releases_direct_control_and_attempts_every_recovery_stage() {
    let io = Dchu::test_io(package, read, set, write);
    for (direct, fail_release, fail_read) in [
        (true, false, false),
        (false, false, false),
        (true, true, false),
        (true, false, true),
    ] {
        *RECORD.lock().unwrap() = Record {
            commands: vec![],
            fail_release,
            fail_read,
        };
        let result = restore_auto(&io, direct);
        assert_eq!(result.is_ok(), !fail_release && !fail_read);
        let record = RECORD.lock().unwrap();
        let commands: Vec<_> = record.commands.iter().map(|c| c.0).collect();
        assert_eq!(
            commands,
            if direct {
                vec![0x69, 121, -1]
            } else {
                vec![121, -1]
            }
        );
        if direct {
            assert_eq!(
                record.commands[0].1,
                crate::hal::firmware::DIRECT_FAN_MASK.to_le_bytes()
            );
        }
        assert!(
            record
                .commands
                .iter()
                .any(|(c, b)| *c == 121 && b == &[0, 0, 0, 1])
        );
    }
}
