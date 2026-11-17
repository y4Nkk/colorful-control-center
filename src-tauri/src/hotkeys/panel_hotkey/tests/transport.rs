use super::*;
use crate::hal::{Hal, oem_dchu::Dchu};
use std::{cell::RefCell, sync::Arc};
#[derive(Default)]
struct Calls {
    queries: usize,
    writes: Vec<(i32, Vec<u8>)>,
    invalid: usize,
    disabled: bool,
    fail: bool,
}
thread_local! { static CALLS: RefCell<Calls> = RefCell::default(); }
unsafe extern "system" fn unused_buffer(_: i32, _: *mut u8) -> i32 {
    CALLS.with_borrow_mut(|c| c.invalid += 1);
    0
}
unsafe extern "system" fn unused_read(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
    CALLS.with_borrow_mut(|c| c.invalid += 1);
    0
}
unsafe extern "system" fn unused_write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    CALLS.with_borrow_mut(|c| c.invalid += 1);
    0
}
unsafe extern "system" fn query(
    command: i32,
    input: *const u8,
    length: i32,
    output: *mut u8,
) -> i32 {
    CALLS.with_borrow_mut(|c| {
        let request = unsafe { std::slice::from_raw_parts(input, length.max(0) as usize) };
        if command != 4
            || request.len() != 256
            || request[0] != 8
            || request[1..].iter().any(|b| *b != 0)
        {
            c.invalid += 1;
            return 0;
        }
        c.queries += 1;
        unsafe {
            std::ptr::write_bytes(output, 0, 257);
            output.write(1);
            output.add(256).write(1);
            output.add(15).write(if c.disabled { 8 } else { 0 });
        }
        4
    })
}
unsafe extern "system" fn send(command: i32, input: *const u8, length: i32) -> i32 {
    CALLS.with_borrow_mut(|c| {
        c.writes.push((command, unsafe {
            std::slice::from_raw_parts(input, length.max(0) as usize).to_vec()
        }));
        if c.fail { 0 } else { command }
    })
}
fn hal() -> Hal {
    CALLS.with_borrow_mut(|c| *c = Calls::default());
    let dchu =
        Dchu::test_io(unused_buffer, unused_read, send, unused_write).with_test_exchange(query);
    let mut hal = Hal::test_instance(dchu, None);
    hal.test_panel_inventory = Some(Arc::new(|| Ok(inventory(false))));
    hal
}
#[test]
fn native_non_oled_queries_live_sub8_and_sends_only_scmd94_u32_one_with_checked_status() {
    let hal = hal();
    let io = native::Native(&hal);
    let source = io.current().unwrap();
    CALLS.with_borrow(|c| {
        assert_eq!(c.queries, 1);
        assert!(c.writes.is_empty());
    });
    io.wake(&source).unwrap();
    CALLS.with_borrow(|c| {
        assert_eq!(c.queries, 3);
        assert_eq!(c.writes, [(94, vec![1, 0, 0, 0])]);
        assert_eq!(c.invalid, 0);
    });
    CALLS.with_borrow_mut(|c| c.fail = true);
    assert!(io.wake(&source).is_err());
    CALLS.with_borrow_mut(|c| {
        c.disabled = true;
        c.writes.clear();
    });
    assert!(io.wake(&source).is_err());
    CALLS.with_borrow(|c| {
        assert!(c.writes.is_empty());
        assert_eq!(c.invalid, 0);
    });
}
#[test]
fn test_native_requires_explicit_inventory_and_never_starts_real_resources() {
    let mut hal = hal();
    hal.test_panel_inventory = None;
    assert!(native::Native(&hal).current().is_err());
    assert!(native::Native(&hal).prepare(&source(false)).is_err());
    hal.test_panel_inventory = Some(Arc::new(|| Ok(inventory(true))));
    assert!(
        native::Native(&hal)
            .current()
            .unwrap_err()
            .to_string()
            .contains("隔离测试")
    );
}
