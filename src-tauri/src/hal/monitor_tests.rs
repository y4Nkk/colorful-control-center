use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
static FAIL: AtomicBool = AtomicBool::new(false);
static MEMORY_READS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static MEMORY_FAIL: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn buffer(command: i32, output: *mut u8) -> i32 {
    if FAIL.load(Ordering::SeqCst) {
        return 0;
    }
    let mut bytes = [0; 257];
    bytes[18] = 60;
    bytes[16] = 100;
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
    }
    command
}
unsafe extern "system" fn read(page: i32, offset: i32, length: i32, output: *mut u8) -> i32 {
    MEMORY_READS.fetch_add(1, Ordering::SeqCst);
    if MEMORY_FAIL.load(Ordering::SeqCst) {
        return 0;
    }
    let bytes = match (page, offset, length) {
        (6, 23, 4) => 2400i32.to_le_bytes().to_vec(),
        (6, 230, 2) => 3200u16.to_le_bytes().to_vec(),
        _ => return 0,
    };
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len()) };
    4096
}
unsafe extern "system" fn set(_: i32, _: *const u8, _: i32) -> i32 {
    0
}
unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    0
}

#[test]
fn current_transport_failure_clears_availability_and_old_readings_then_can_recover() {
    let hal = Hal::test_instance(oem_dchu::Dchu::test_io(buffer, read, set, write), None);
    FAIL.store(false, Ordering::SeqCst);
    MEMORY_READS.store(0, Ordering::SeqCst);
    let first = hal.snapshot();
    assert!(first.dchu_available);
    assert_eq!(first.cpu_temp_c, Some(60.0));
    assert_eq!(first.oem_memory.unwrap().reported_clock_mhz, Some(2400));
    assert_eq!(MEMORY_READS.load(Ordering::SeqCst), 2);
    FAIL.store(true, Ordering::SeqCst);
    let failed = hal.snapshot();
    assert!(!failed.dchu_available);
    assert!(failed.cpu_temp_c.is_none());
    assert!(failed.fan.cpu_rpm.is_none());
    assert!(failed.oem_memory.is_none());
    assert_eq!(MEMORY_READS.load(Ordering::SeqCst), 2);
    assert!(
        failed
            .warnings
            .iter()
            .any(|warning| warning.contains("传感读取失败"))
    );
    FAIL.store(false, Ordering::SeqCst);
    let recovered = hal.snapshot();
    assert!(recovered.dchu_available);
    assert_eq!(recovered.cpu_temp_c, Some(60.0));
    assert_eq!(recovered.oem_memory.unwrap().reported_clock_mhz, Some(2400));
    assert_eq!(MEMORY_READS.load(Ordering::SeqCst), 4);
}
#[test]
fn isolated_missing_providers_never_load_real_dll_or_nvml() {
    let hal = Hal::test_instance(oem_dchu::Dchu::test_io(buffer, read, set, write), None);
    hal.dchu.lock().unwrap().take();
    let result = hal.snapshot();
    assert!(!result.dchu_available);
    assert!(result.nvidia.is_none());
    assert!(result.oem_memory.is_none());
    assert!(hal.dchu.lock().unwrap().is_none());
    assert!(hal.nvidia.lock().unwrap().is_none());
}
#[test]
fn power_suspend_preserves_cpu_monitoring_and_marks_gpu_as_paused() {
    let hal = Hal::test_instance(oem_dchu::Dchu::test_io(buffer, read, set, write), None);
    FAIL.store(false, Ordering::SeqCst);
    hal.suspend_gpu(true).unwrap();
    let snapshot = hal.snapshot();
    assert!(snapshot.dchu_available);
    assert_eq!(snapshot.cpu_temp_c, Some(60.0));
    assert_eq!(snapshot.oem_memory.unwrap().reported_clock_mhz, Some(2400));
    assert!(snapshot.nvidia_suspended);
    assert!(snapshot.nvidia.is_none());
    assert!(hal.nvidia.lock().unwrap().is_none());
    hal.suspend_gpu(false).unwrap();
    assert!(!hal.snapshot().nvidia_suspended);
}

#[test]
fn shared_memory_failure_keeps_current_cpu_and_fan_transport_available() {
    let hal = Hal::test_instance(oem_dchu::Dchu::test_io(buffer, read, set, write), None);
    FAIL.store(false, Ordering::SeqCst);
    MEMORY_FAIL.store(true, Ordering::SeqCst);
    let failed = hal.snapshot();
    MEMORY_FAIL.store(false, Ordering::SeqCst);
    assert!(failed.dchu_available);
    assert_eq!(failed.cpu_temp_c, Some(60.0));
    assert!(failed.fan.cpu_rpm.is_some());
    assert!(failed.oem_memory.is_none());
    assert_eq!(failed.warnings.len(), 2);
    assert!(hal.snapshot().oem_memory.is_some());
}

#[test]
fn isolated_keyboard_inventory_requires_explicit_provider_and_keeps_errors() {
    let mut hal = Hal::test_instance(oem_dchu::Dchu::test_io(buffer, read, set, write), None);
    assert!(
        hal.usb_keyboard_present()
            .unwrap_err()
            .to_string()
            .contains("禁止回退系统查询")
    );
    for expected in [false, true] {
        hal.test_usb_keyboard = Some(std::sync::Arc::new(move || Ok(expected)));
        assert_eq!(hal.usb_keyboard_present().unwrap(), expected);
    }
    hal.test_usb_keyboard = Some(std::sync::Arc::new(|| {
        anyhow::bail!("fixture query failed")
    }));
    assert_eq!(
        hal.usb_keyboard_present().unwrap_err().to_string(),
        "fixture query failed"
    );
}
