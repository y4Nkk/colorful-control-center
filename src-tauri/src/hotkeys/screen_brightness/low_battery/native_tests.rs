use super::*;
use crate::hal::{Hal, oem_dchu::Dchu};
use std::cell::RefCell;

thread_local! {
    static WRITES: RefCell<Vec<(i32, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
    static CACHE: RefCell<[u8; 2]> = const { RefCell::new([0, 17]) };
}
unsafe extern "system" fn packet(command: i32, output: *mut u8) -> i32 {
    if command != 17 {
        return 0;
    }
    let mut data = [0u8; 257];
    data[2] = 5;
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), output, data.len()) };
    command
}
unsafe extern "system" fn read(page: i32, offset: i32, length: i32, output: *mut u8) -> i32 {
    if (page, offset, length) != (1, 13, 2) {
        return 0;
    }
    CACHE.with_borrow(|cache| unsafe { std::ptr::copy_nonoverlapping(cache.as_ptr(), output, 2) });
    4096
}
unsafe extern "system" fn send(command: i32, input: *const u8, length: i32) -> i32 {
    if command != 121 || length != 4 {
        return 0;
    }
    WRITES.with_borrow_mut(|writes| {
        writes.push((
            command,
            unsafe { std::slice::from_raw_parts(input, 4) }.to_vec(),
        ))
    });
    command
}
unsafe extern "system" fn write(page: i32, offset: i32, length: i32, input: *const u8) -> i32 {
    if page != 1 || !(13..=14).contains(&offset) || length != 1 {
        return 0;
    }
    CACHE.with_borrow_mut(|cache| cache[(offset - 13) as usize] = unsafe { *input });
    1
}
unsafe extern "system" fn exchange(
    command: i32,
    input: *const u8,
    length: i32,
    output: *mut u8,
) -> i32 {
    if command != 4 || length != 256 || unsafe { *input } != 8 {
        return 0;
    }
    let mut data = [0u8; 257];
    data[0] = 1;
    data[16] = 0x40;
    data[256] = 1;
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), output, data.len()) };
    4
}

#[test]
fn native_adapter_uses_injected_exact_packets_and_never_falls_back_to_system_power_or_owner() {
    WRITES.with_borrow_mut(Vec::clear);
    CACHE.with_borrow_mut(|cache| *cache = [0, 17]);
    let dchu = Dchu::test_io(packet, read, send, write).with_test_exchange(exchange);
    let hal = Hal::test_instance(dchu, None);
    let native = Native::new(&hal);
    let rule = native.rule().unwrap();
    assert_eq!(rule.version, [1, 0]);
    assert_eq!(rule.customer, 5);
    assert!(rule.feature);
    assert_eq!(native.cache().unwrap(), [0, 17]);
    native.write_cache(0, 80).unwrap();
    native.write_cache(1, 20).unwrap();
    assert_eq!(native.cache().unwrap(), [80, 20]);
    native.signal(2).unwrap();
    native.signal(3).unwrap();
    assert!(native.signal(1).is_err());
    WRITES.with_borrow(|writes| {
        assert_eq!(
            *writes,
            [(121, vec![2, 0, 0, 44]), (121, vec![3, 0, 0, 44])]
        )
    });
    assert!(native.power().is_err());
    assert!(native.ensure_writable().is_err());
}
