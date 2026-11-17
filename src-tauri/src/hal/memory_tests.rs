use super::*;
use std::cell::RefCell;

#[derive(Default)]
struct State {
    clock: i32,
    reference: u16,
    faults: u8,
    reads: Vec<(i32, i32, i32)>,
    unexpected: usize,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

unsafe extern "system" fn read(page: i32, offset: i32, length: i32, output: *mut u8) -> i32 {
    STATE.with_borrow_mut(|state| {
        state.reads.push((page, offset, length));
        let bytes = match (page, offset, length) {
            (6, 23, 4) if state.faults & 1 == 0 => state.clock.to_le_bytes().to_vec(),
            (6, 230, 2) if state.faults & 2 == 0 => state.reference.to_le_bytes().to_vec(),
            _ => return 0,
        };
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len()) };
        4096
    })
}
unsafe extern "system" fn buffer(_: i32, _: *mut u8) -> i32 {
    STATE.with_borrow_mut(|state| state.unexpected += 1);
    0
}
unsafe extern "system" fn set(_: i32, _: *const u8, _: i32) -> i32 {
    STATE.with_borrow_mut(|state| state.unexpected += 1);
    0
}
unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    STATE.with_borrow_mut(|state| state.unexpected += 1);
    0
}

fn sample(clock: i32, reference: u16, faults: u8) -> (Option<OemMemoryReading>, Vec<String>) {
    STATE.with_borrow_mut(|state| {
        *state = State {
            clock,
            reference,
            faults,
            ..Default::default()
        };
    });
    let io = Dchu::test_io(buffer, read, set, write);
    let mut warnings = Vec::new();
    let result = super::read(&io, &mut warnings);
    STATE.with_borrow(|state| {
        assert_eq!(state.reads, [(6, 23, 4), (6, 230, 2)]);
        assert_eq!(
            state.unexpected, 0,
            "memory monitoring only reads exact shared fields"
        );
    });
    (result, warnings)
}

#[test]
fn signed_little_endian_clock_and_unsigned_reference_are_not_clamped_or_scaled() {
    for (clock, reference) in [(4800, 3200), (0x12345, 0x4321), (i32::MAX, u16::MAX)] {
        let (result, warnings) = sample(clock, reference, 0);
        let result = result.unwrap();
        assert_eq!(result.reported_clock_mhz, Some(clock as u32));
        assert_eq!(result.reference_clock_mhz, Some(reference as u32));
        assert!(warnings.is_empty());
    }
}

#[test]
fn zero_is_unreported_and_each_field_can_remain_unknown() {
    let (result, warnings) = sample(0, 0, 0);
    assert!(result.is_none());
    assert!(warnings.is_empty());
    let result = sample(2400, 0, 0).0.unwrap();
    assert_eq!(result.reported_clock_mhz, Some(2400));
    assert!(result.reference_clock_mhz.is_none());
    let result = sample(0, 3200, 0).0.unwrap();
    assert!(result.reported_clock_mhz.is_none());
    assert_eq!(result.reference_clock_mhz, Some(3200));
}

#[test]
fn negative_clock_never_becomes_a_large_unsigned_frequency() {
    for clock in [-1, i32::MIN] {
        let (result, warnings) = sample(clock, 3200, 0);
        let result = result.unwrap();
        assert!(result.reported_clock_mhz.is_none());
        assert_eq!(result.reference_clock_mhz, Some(3200));
        assert_eq!(warnings, [format!("内存原厂报告频率无效：{clock}")]);
    }
    assert!(sample(-1, 0, 0).0.is_none());
}

#[test]
fn independent_read_failures_preserve_other_report_and_diagnostics() {
    let (result, warnings) = sample(2400, 3200, 1);
    let result = result.unwrap();
    assert!(result.reported_clock_mhz.is_none());
    assert_eq!(result.reference_clock_mhz, Some(3200));
    assert!(warnings[0].contains("报告频率读取失败"));
    assert!(warnings[0].contains("status=0x0"));
    let (result, warnings) = sample(2400, 3200, 2);
    let result = result.unwrap();
    assert_eq!(result.reported_clock_mhz, Some(2400));
    assert!(result.reference_clock_mhz.is_none());
    assert!(warnings[0].contains("参考频率读取失败"));
    let (result, warnings) = sample(2400, 3200, 3);
    assert!(result.is_none());
    assert_eq!(warnings.len(), 2);
}

#[test]
fn snapshot_serialization_has_one_nullable_camel_case_memory_contract() {
    let reading = sample(2400, 0, 0).0;
    let snapshot = crate::hal::HardwareSnapshot {
        oem_memory: reading,
        ..Default::default()
    };
    let json = serde_json::to_value(snapshot).unwrap();
    assert_eq!(json["oemMemory"]["reportedClockMhz"], 2400);
    assert!(json["oemMemory"]["referenceClockMhz"].is_null());
    assert!(json.get("oem_memory").is_none());
    let empty = serde_json::to_value(crate::hal::HardwareSnapshot::default()).unwrap();
    assert!(empty["oemMemory"].is_null());
}
