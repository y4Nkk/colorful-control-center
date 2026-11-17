use super::*;
use crate::hal::oem_dchu::Dchu;
use std::cell::RefCell;

thread_local! { static REQUESTS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) }; }
unsafe extern "system" fn packet(_: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn settings(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn send(_: i32, _: *const u8, _: i32) -> i32 {
    0
}
unsafe extern "system" fn save(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
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
    let request = unsafe { std::slice::from_raw_parts(input, length as usize) };
    REQUESTS.with_borrow_mut(|requests| requests.push(request.to_vec()));
    let mut response = [0; 257];
    match request[0] {
        8 => {
            response[0] = 1;
            response[17] = 0x10;
        }
        1 => {
            response[1..3].copy_from_slice(&2832u16.to_le_bytes());
        }
        _ => return 0,
    }
    response[256] = response[0];
    unsafe {
        std::ptr::copy_nonoverlapping(response.as_ptr(), output, response.len());
    }
    4
}
#[test]
fn native_adapter_uses_the_exact_injected_ec_query_and_never_falls_back_to_system_power() {
    REQUESTS.with_borrow_mut(Vec::clear);
    let dchu = Dchu::test_io(packet, settings, send, save).with_test_exchange(exchange);
    let hal = Hal::test_instance(dchu, None);
    assert!(identity(&hal).is_ok());
    assert_eq!(temperature(&Native(&hal)).unwrap(), 10);
    assert!(Native(&hal).power().is_err());
    REQUESTS.with_borrow(|requests| {
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0][0], 8);
        assert_eq!(&requests[1][..7], &[1, 12, 90, 22, 8, 0, 223]);
        assert!(requests[1][7..].iter().all(|value| *value == 0));
    });
}
