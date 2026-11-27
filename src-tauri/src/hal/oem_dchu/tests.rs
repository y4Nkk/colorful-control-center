use super::*;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::mem::ManuallyDrop;
use std::time::Duration;

unsafe extern "system" fn integer(command: i32, output: *mut i32) -> i32 {
    unsafe {
        output.write(command + 1);
    }
    command + 1
}

unsafe extern "system" fn packet(command: i32, output: *mut u8) -> i32 {
    // 模拟本机 DLL 的 256B 负载 + 首字节额外复制，不访问任何原厂导出。
    unsafe {
        std::ptr::write_bytes(output, 0, 257);
        output.add(1).write(0x42);
    }
    command
}

unsafe extern "system" fn exchange(command: i32, _: *const u8, _: i32, output: *mut u8) -> i32 {
    unsafe { packet(command, output) }
}

unsafe extern "system" fn read(_: i32, _: i32, length: i32, output: *mut u8) -> i32 {
    unsafe {
        std::ptr::write_bytes(output, 0x42, length as usize);
    }
    4096
}

unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    1
}
unsafe extern "system" fn set(command: i32, _: *const u8, length: i32) -> i32 {
    match (command, length) {
        (0x0e, 256) => 0x14,
        (_, 4 | 256) => command,
        _ => 0,
    }
}
unsafe extern "system" fn no_packet(_: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn no_set(_: i32, _: *const u8, _: i32) -> i32 {
    0
}
unsafe extern "system" fn no_read(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn no_write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    0
}
unsafe extern "system" fn empty_exchange(command: i32, _: *const u8, _: i32, _: *mut u8) -> i32 {
    command
}

fn fake() -> ManuallyDrop<Dchu> {
    ManuallyDrop::new(Dchu {
        handle: std::ptr::null_mut(),
        get_int: integer,
        get_buf: packet,
        set_data: set,
        set_data_ex: Some(exchange),
        read_settings: read,
        write_settings: write,
        acknowledged_mode: Default::default(),
        fan_mode_unconfirmed: Default::default(),
        test_entertainment_options: Default::default(),
    })
}

#[test]
fn native_set_returns_the_firmware_result() {
    let io = fake();
    assert_eq!(io.read_int(16), (true, 17));
    assert_eq!(io.read_package(12).unwrap()[1], 0x42);
    assert_eq!(io.exchange_package(4, &[0; 256]).unwrap()[1], 0x42);
    io.set_data_sub(121, 1, 6).unwrap();
    io.set_u32(39, 3).unwrap();
    io.set_package(14, &[0; 256]).unwrap();
}

thread_local! {
    static FAN_READS: RefCell<VecDeque<(Duration, u8)>> = const { RefCell::new(VecDeque::new()) };
    static FAN_WRITES: Cell<usize> = const { Cell::new(0) };
}

fn scripted_fan(reads: &[(Duration, u8)]) -> ManuallyDrop<Dchu> {
    unsafe extern "system" fn mode(command: i32, out: *mut u8) -> i32 {
        let Some((delay, bits)) = FAN_READS.with_borrow_mut(VecDeque::pop_front) else {
            return 0;
        };
        std::thread::sleep(delay);
        let mut report = [0u8; 257];
        report[12] = 2;
        report[14] = bits;
        unsafe { std::ptr::copy_nonoverlapping(report.as_ptr(), out, report.len()) };
        command
    }
    unsafe extern "system" fn set_mode(command: i32, _: *const u8, _: i32) -> i32 {
        FAN_WRITES.set(FAN_WRITES.get() + 1);
        command
    }
    FAN_READS.set(reads.iter().copied().collect());
    FAN_WRITES.set(0);
    let mut io = fake();
    io.get_buf = mode;
    io.set_data = set_mode;
    io
}

#[test]
fn slow_first_matching_read_cannot_confirm_an_uncertain_fan_restore() {
    let io = scripted_fan(&[(fan::MODE_CONFIRM_TIMEOUT, 0x08), (Duration::ZERO, 0xff)]);
    let error = io.force_restore_fan_mode(2).unwrap_err();
    assert!(format!("{error:#}").contains("0xff"), "{error:#}");
    assert!(FAN_READS.with_borrow(VecDeque::is_empty));
    assert_eq!(FAN_WRITES.get(), 1);
}

#[test]
fn a_first_target_at_the_deadline_requires_a_second_matching_read() {
    for (next, confirmed) in [(0x02, false), (0x08, true)] {
        let io = scripted_fan(&[
            (Duration::ZERO, 0x02),
            (fan::MODE_CONFIRM_TIMEOUT, 0x08),
            (Duration::ZERO, next),
        ]);
        let result = io.force_restore_fan_mode(2);
        assert_eq!(result.is_ok(), confirmed, "{result:#?}");
        assert!(FAN_READS.with_borrow(VecDeque::is_empty));
        assert_eq!(FAN_WRITES.get(), 1);
    }
}

#[test]
fn a_slow_first_wrong_read_can_still_establish_a_stable_mismatch_and_retry() {
    let io = scripted_fan(&[
        (fan::MODE_CONFIRM_TIMEOUT, 0x08),
        (Duration::ZERO, 0x08),
        (Duration::ZERO, 0x02),
        (Duration::ZERO, 0x02),
    ]);
    io.set_fan_mode_verified(0).unwrap();
    assert!(FAN_READS.with_borrow(VecDeque::is_empty));
    assert_eq!(FAN_WRITES.get(), 2);
}

#[test]
fn two_point_command_echo_is_rejected() {
    unsafe extern "system" fn echo(command: i32, _: *const u8, _: i32) -> i32 {
        command
    }
    let mut io = fake();
    io.set_data = echo;
    assert!(io.set_package(14, &[0; 256]).is_err());
    io.set_package(4, &[0; 256]).unwrap();
}

#[test]
fn integer_result_is_the_payload_and_unwritten_zero_is_failure() {
    unsafe extern "system" fn zero(_: i32, output: *mut i32) -> i32 {
        unsafe { output.write(0) };
        0
    }
    unsafe extern "system" fn failed(_: i32, _: *mut i32) -> i32 {
        0
    }
    unsafe extern "system" fn echo(command: i32, output: *mut i32) -> i32 {
        unsafe { output.write(command) };
        command
    }
    let mut io = fake();
    io.get_int = zero;
    assert_eq!(io.read_int(3), (true, 0));
    io.get_int = failed;
    assert!(!io.read_int(3).0);
    io.get_int = echo;
    assert_eq!(io.read_int(3), (false, 3));
}

#[test]
fn zero_return_never_fabricates_a_successful_operation() {
    let mut io = fake();
    io.get_buf = no_packet;
    io.set_data = no_set;
    io.read_settings = no_read;
    io.write_settings = no_write;
    assert!(io.read_package(12).is_err());
    assert!(io.set_data_sub(121, 1, 6).is_err());
    assert!(io.set_u32(39, 3).is_err());
    assert!(io.set_package(14, &[0; 256]).is_err());
    assert!(io.read_settings(4, 5, 1).is_err());
    assert!(io.write_settings(4, 5, &[6]).is_err());
}

#[test]
fn appsettings_uses_its_own_read_and_write_return_conventions() {
    let io = fake();
    assert_eq!(io.read_settings(4, 5, 2).unwrap(), [0x42, 0x42]);
    io.write_settings(4, 5, &[6]).unwrap();
    assert!(check_status(1, 4096, "read").is_err());
    assert!(check_status(4096, 1, "write").is_err());
    assert!(check_status(-1, 121, "set").is_err());
}

#[test]
fn exchange_returning_a_command_without_output_is_not_success() {
    let mut io = fake();
    io.set_data_ex = Some(empty_exchange);
    let error = io.exchange_package(4, &[0; 256]).unwrap_err().to_string();
    assert!(error.contains("未返回完整"));
    io.set_data_ex = None;
    assert!(io.exchange_package(4, &[0; 256]).is_err());
}

#[test]
fn packet_output_reserves_the_extra_byte_and_keeps_only_payload() {
    let mut output = PacketOutput::new();
    assert!(output.bytes.len() >= 257);
    assert_ne!(output.bytes[0], output.bytes[256]);
    unsafe {
        packet(12, output.bytes.as_mut_ptr());
    }
    assert_eq!(output.bytes[256], output.bytes[0]);
    let payload = output.finish(12, 12, "read").unwrap();
    assert_eq!(payload.len(), 256);
    assert_eq!(payload[1], 0x42);
    assert!(PacketOutput::new().finish(12, 13, "read").is_err());
}

#[test]
fn appsettings_stays_inside_the_native_4096_byte_store() {
    assert!(validate_settings_range(15, 255, 1).is_ok());
    for (page, offset, length) in [(16, 0, 1), (-1, 0, 1), (4, -1, 1), (4, 255, 2), (4, 0, 0)] {
        assert!(validate_settings_range(page, offset, length).is_err());
    }
}
