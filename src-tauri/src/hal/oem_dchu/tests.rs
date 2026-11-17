use super::*;
use std::mem::ManuallyDrop;

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
    if [4, 256].contains(&length) {
        command
    } else {
        0
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
    })
}

#[test]
fn command_echo_is_success_for_the_reviewed_native_exports() {
    let io = fake();
    assert_eq!(io.read_int(16), (true, 17));
    assert_eq!(io.read_package(12).unwrap()[1], 0x42);
    assert_eq!(io.exchange_package(4, &[0; 256]).unwrap()[1], 0x42);
    io.set_data_sub(121, 1, 6).unwrap();
    io.set_u32(39, 3).unwrap();
    io.set_package(14, &[0; 256]).unwrap();
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
