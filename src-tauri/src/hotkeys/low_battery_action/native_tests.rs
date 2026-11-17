use super::*;
use crate::hal::oem_dchu::Dchu;
use std::cell::RefCell;

thread_local! { static REQUESTS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) }; }
unsafe extern "system" fn packet(_: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn read(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn send(_: i32, _: *const u8, _: i32) -> i32 {
    0
}
unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    0
}
unsafe extern "system" fn exchange(
    command: i32,
    input: *const u8,
    length: i32,
    output: *mut u8,
) -> i32 {
    if command != 4 || length != 256 {
        return 0;
    }
    let request = unsafe { std::slice::from_raw_parts(input, 256) };
    REQUESTS.with_borrow_mut(|items| items.push(request.to_vec()));
    let response = [0u8; 257];
    unsafe { std::ptr::copy_nonoverlapping(response.as_ptr(), output, response.len()) };
    4
}
#[test]
fn native_adapter_emits_exact_injected_packet_without_real_system_action_or_power_access() {
    REQUESTS.with_borrow_mut(Vec::clear);
    let dchu = Dchu::test_io(packet, read, send, write).with_test_exchange(exchange);
    let hal = Hal::test_instance(dchu, None);
    let native = Native(&hal);
    native.signal(1).unwrap();
    native.signal(2).unwrap();
    assert!(native.signal(0).is_err());
    REQUESTS.with_borrow(|items| {
        assert_eq!(items.len(), 2);
        for (request, level) in items.iter().zip([1, 2]) {
            assert_eq!(&request[..2], &[46, level]);
            assert!(request[2..].iter().all(|value| *value == 0));
        }
    });
    assert!(native.power().is_err());
    assert!(native.ensure_writable().is_err());
    assert!(native.suspend(false).is_err());
    assert!(native.suspend(true).is_err());
    assert!(native.active().is_err());
}
