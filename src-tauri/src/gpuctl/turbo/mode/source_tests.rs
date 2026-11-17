use super::*;
use crate::hal::oem_dchu::Dchu;
use std::sync::{Mutex, MutexGuard};

struct Data {
    packet: [u8; 256],
    page: [u8; 256],
    reads: Vec<(i32, i32, i32)>,
    integers: Vec<i32>,
    echo16: bool,
    writes: usize,
}
static DATA: Mutex<Option<Data>> = Mutex::new(None);
static SERIAL: Mutex<()> = Mutex::new(());

unsafe extern "system" fn buffer(_: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn exchange(command: i32, input: *const u8, _: i32, out: *mut u8) -> i32 {
    let data = DATA.lock().unwrap();
    let data = data.as_ref().unwrap();
    let mut value = [0u8; 257];
    match unsafe { *input } {
        8 => value[..256].copy_from_slice(&data.packet),
        9 => value[4..6].copy_from_slice(&1700u16.to_le_bytes()),
        _ => return 0,
    }
    value[256] = value[0];
    unsafe {
        std::ptr::copy_nonoverlapping(value.as_ptr(), out, value.len());
    }
    command
}
unsafe extern "system" fn read(page: i32, offset: i32, length: i32, out: *mut u8) -> i32 {
    let mut data = DATA.lock().unwrap();
    let data = data.as_mut().unwrap();
    data.reads.push((page, offset, length));
    if page != 5 {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(data.page[offset as usize..].as_ptr(), out, length as usize);
    }
    4096
}
unsafe extern "system" fn integer(command: i32, out: *mut i32) -> i32 {
    let mut data = DATA.lock().unwrap();
    let data = data.as_mut().unwrap();
    data.integers.push(command);
    let value = if command == 16 && data.echo16 {
        command
    } else {
        0x20
    };
    unsafe {
        *out = value;
    }
    value
}
unsafe extern "system" fn set(command: i32, _: *const u8, _: i32) -> i32 {
    DATA.lock().unwrap().as_mut().unwrap().writes += 1;
    command
}
unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    DATA.lock().unwrap().as_mut().unwrap().writes += 1;
    1
}

fn setup(version: u8) -> (MutexGuard<'static, ()>, Hal) {
    let serial = SERIAL.lock().unwrap();
    let mut packet = [0; 256];
    packet[0] = version;
    let mut page = [0; 256];
    page[0..2].copy_from_slice(&0x28a0u16.to_le_bytes());
    page[6] = 2;
    page[9] = 1;
    page[16..20].copy_from_slice(&[100, 0, 254, 255]);
    *DATA.lock().unwrap() = Some(Data {
        packet,
        page,
        reads: vec![],
        integers: vec![],
        echo16: false,
        writes: 0,
    });
    let io = Dchu::test_io(buffer, read, set, write)
        .with_test_exchange(exchange)
        .with_test_integer(integer);
    (serial, Hal::test_instance(io, None))
}

fn model() -> Model {
    Model {
        product: 1,
        customer: 5,
        dc_mode_power: true,
    }
}

#[test]
fn nonempty_v1_reads_the_original_cache_and_never_initializes_or_writes_it() {
    let (_serial, hal) = setup(1);
    {
        let mut data = DATA.lock().unwrap();
        let data = data.as_mut().unwrap();
        data.packet[2] = 1;
        data.packet[13] = 0x20;
        data.packet[15] = 1;
        data.packet[20] = 8;
    }
    let result = super::read(&hal, &model()).unwrap();
    assert_eq!(result.cached_device_id, 0x28a0);
    assert_eq!(result.gpu_count, 2);
    assert!(result.ready && result.simple_gpu_oc && result.capabilities.unwrap().limit_clock);
    assert_eq!(
        result.saved[0],
        Offsets {
            core_mhz: 100,
            memory_mhz: -1
        }
    );
    let data = DATA.lock().unwrap();
    let data = data.as_ref().unwrap();
    assert_eq!(data.reads, [(5, 0, 13), (5, 16, 8)]);
    assert!(data.integers.is_empty());
    assert_eq!(data.writes, 0);
}

#[test]
fn v0_and_empty_v1_read_both_legacy_capabilities_without_initializing_settings() {
    for version in [0, 1] {
        let (_serial, hal) = setup(version);
        let result = super::read(&hal, &model()).unwrap();
        assert_eq!(result.gpu_oc, Some(true));
        assert!(result.capabilities.unwrap().all_mode_oc);
        let data = DATA.lock().unwrap();
        let data = data.as_ref().unwrap();
        assert_eq!(data.integers, [16, 96]);
        assert_eq!(data.writes, 0);
    }
}

#[test]
fn a_legacy_command_echo_is_unknown_instead_of_a_confirmed_gpu_capability() {
    let (_serial, hal) = setup(0);
    DATA.lock().unwrap().as_mut().unwrap().echo16 = true;
    let result = super::read(&hal, &model()).unwrap();
    assert_eq!(result.gpu_oc, None);
    assert!(result.capabilities.unwrap().all_mode_oc);
    assert_eq!(DATA.lock().unwrap().as_ref().unwrap().writes, 0);
}

#[test]
fn unknown_feature_versions_stop_before_settings_reads_or_any_output() {
    let (_serial, hal) = setup(2);
    assert!(super::read(&hal, &model()).is_err());
    let data = DATA.lock().unwrap();
    let data = data.as_ref().unwrap();
    assert!(data.reads.is_empty() && data.integers.is_empty());
    assert_eq!(data.writes, 0);
}
