use super::*;
use firmware::DirectControl;
use std::sync::Mutex;

struct Record {
    commands: Vec<(i32, Vec<u8>)>,
    fail_release: bool,
    fail_read: bool,
    skip_release: bool,
    control: [DirectControl; 2],
    mode_bits: u8,
}
static RECORD: Mutex<Record> = Mutex::new(Record {
    commands: Vec::new(),
    fail_release: false,
    fail_read: false,
    skip_release: false,
    control: [DirectControl::Firmware; 2],
    mode_bits: 2,
});

fn direct_control(_: &Dchu) -> Result<[DirectControl; 2]> {
    Ok(RECORD.lock().unwrap().control)
}

unsafe extern "system" fn package(command: i32, out: *mut u8) -> i32 {
    assert_eq!(command, 13);
    let mut data = [0; 257];
    data[12] = 2;
    data[14] = RECORD.lock().unwrap().mode_bits;
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), out, data.len()) };
    command
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
    if command == 0x69 && !record.skip_release {
        record.control = [DirectControl::Firmware; 2];
    }
    command
}

#[test]
fn child_restore_releases_direct_control_and_attempts_every_recovery_stage() {
    let io = Dchu::test_io(package, read, set, write);
    for (has_reader, fail_release, fail_read, skip_release, mode_bits) in [
        (true, false, false, false, 2),
        (false, false, false, false, 2),
        (true, true, false, false, 2),
        (true, false, true, false, 2),
        (true, false, false, true, 2),
        (true, false, false, false, 16),
    ] {
        *RECORD.lock().unwrap() = Record {
            commands: vec![],
            fail_release,
            fail_read,
            skip_release,
            control: [DirectControl::Manual(90), DirectControl::Firmware],
            mode_bits,
        };
        let reader = has_reader.then_some(direct_control as firmware::ReadDirectControl);
        let result = restore_auto(&io, reader);
        assert_eq!(
            result.is_ok(),
            has_reader && !fail_release && !fail_read && !skip_release && mode_bits == 2
        );
        if skip_release {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("尚未确认所有风扇已退出软件调速")
            );
        }
        let record = RECORD.lock().unwrap();
        let commands: Vec<_> = record.commands.iter().map(|c| c.0).collect();
        assert_eq!(commands, vec![0x69, 121, -1]);
        assert_eq!(
            record.commands[0].1,
            firmware::DIRECT_FAN_MASK.to_le_bytes()
        );
        assert!(
            record
                .commands
                .iter()
                .any(|(c, b)| *c == 121 && b == &[0, 0, 0, 1])
        );
    }
}
