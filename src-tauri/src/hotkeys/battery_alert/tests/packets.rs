use super::super::native;
use crate::hal::oem_dchu::Dchu;
use std::cell::Cell;
thread_local! { static VALUE: Cell<i32> = const { Cell::new(0) }; static ECHO: Cell<bool> = const { Cell::new(false) }; }
unsafe extern "system" fn integer(command: i32, output: *mut i32) -> i32 {
    assert_eq!(command, 70);
    let value = if ECHO.with(Cell::get) {
        command
    } else {
        VALUE.with(Cell::get)
    };
    unsafe { output.write(value) };
    value
}
unsafe extern "system" fn packet(_: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn read(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    0
}
unsafe extern "system" fn set(_: i32, _: *const u8, _: i32) -> i32 {
    0
}
unsafe extern "system" fn exchange(
    command: i32,
    input: *const u8,
    length: i32,
    output: *mut u8,
) -> i32 {
    assert_eq!((command, length), (4, 256));
    let request = unsafe { std::slice::from_raw_parts(input, 256) };
    assert_eq!(&request[..7], &[1, 5, 0x51, 1, 0, 0, 177]);
    assert!(request[7..].iter().all(|b| *b == 0));
    unsafe {
        std::ptr::write_bytes(output, 0, 257);
        output.add(1).write(VALUE.with(Cell::get) as u8);
    }
    4
}
fn io() -> Dchu {
    Dchu::test_io(packet, read, set, write)
        .with_test_integer(integer)
        .with_test_exchange(exchange)
}
#[test]
fn pc_beep_uses_wmi70_bit13_and_rejects_echo_without_native_fallback() {
    ECHO.set(false);
    for value in [0, 1, 1 << 12, 1 << 13, -1] {
        VALUE.set(value);
        assert_eq!(
            native::pc_beep(&io()).unwrap(),
            value as u32 & (1 << 13) != 0
        );
    }
    ECHO.set(true);
    assert!(native::pc_beep(&io()).is_err());
    ECHO.set(false);
}
#[test]
fn ec_read_uses_exact_original_ecram_packet_and_only_bits_four_and_five() {
    for value in [0, 1, 0x10, 0x20, 0x40, 0xff] {
        VALUE.set(value);
        assert_eq!(native::low_bits(&io()).unwrap(), value & 0x30 != 0);
    }
}
