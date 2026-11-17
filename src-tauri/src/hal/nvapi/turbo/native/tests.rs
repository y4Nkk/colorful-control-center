use super::*;
use crate::hal::nvapi::{NvApi, NvApiSession, ffi::Pstates20Info};
use std::sync::{Arc, Mutex, OnceLock};

struct Fake {
    table: clock::LockTable,
    missing: Option<u32>,
    status: i32,
    reverse: bool,
    get_version: u32,
    frequency_input: (u32, u32),
    writes: Vec<(usize, clock::LockTable)>,
    offsets: [[i32; 2]; 2],
    offset_writes: Vec<(usize, u32, u32, i32)>,
}
impl Default for Fake {
    fn default() -> Self {
        let mut table = clock::LockTable {
            count: 3,
            ..Default::default()
        };
        table.entries[0] = clock::LockEntry {
            domain: 0,
            mode: 3,
            value: 900_000,
            ..Default::default()
        };
        table.entries[1] = clock::LockEntry {
            domain: 1,
            mode: 0,
            ..Default::default()
        };
        table.entries[2] = clock::LockEntry {
            domain: 4,
            mode: 3,
            value: 42,
            ..Default::default()
        };
        Self {
            table,
            missing: None,
            status: 0,
            reverse: false,
            get_version: 0,
            frequency_input: (0, 0),
            writes: Vec::new(),
            offsets: [[10_000, 20_000], [-5_000, 30_000]],
            offset_writes: Vec::new(),
        }
    }
}
static SERIAL: Mutex<()> = Mutex::new(());
static STATE: OnceLock<Mutex<Fake>> = OnceLock::new();
fn state() -> &'static Mutex<Fake> {
    STATE.get_or_init(|| Mutex::new(Fake::default()))
}

unsafe extern "system" fn query(id: u32) -> *mut c_void {
    if state().lock().unwrap().missing == Some(id) {
        return std::ptr::null_mut();
    }
    let pointer = match id {
        clock::GET_PCI_IDENTIFIERS => pci as *const (),
        clock::GET_BUS_ID => bus as *const (),
        clock::GET_BUS_SLOT_ID => slot as *const (),
        clock::GET_LOCKS => get_locks as *const (),
        clock::SET_LOCKS => set_locks as *const (),
        clock::GET_FREQUENCIES => frequencies as *const (),
        _ => std::ptr::null(),
    };
    pointer as *mut c_void
}
unsafe extern "system" fn init() -> i32 {
    0
}
unsafe extern "system" fn unload() -> i32 {
    0
}
unsafe extern "system" fn error_message(_: i32, output: *mut u8) -> i32 {
    unsafe {
        *output = 0;
    }
    0
}
unsafe extern "system" fn enumerate(handles: *mut *mut c_void, count: *mut u32) -> i32 {
    let reverse = state().lock().unwrap().reverse;
    unsafe {
        *count = 2;
        *handles = (if reverse { 2 } else { 1 }) as *mut c_void;
        *handles.add(1) = (if reverse { 1 } else { 2 }) as *mut c_void;
    }
    0
}
unsafe extern "system" fn name(_: *mut c_void, output: *mut u8) -> i32 {
    unsafe {
        *output = 0;
    }
    0
}
unsafe extern "system" fn get_pstates(handle: *mut c_void, output: *mut Pstates20Info) -> i32 {
    let index = (handle as usize).wrapping_sub(1);
    if index >= 2 {
        return -5;
    }
    let values = state().lock().unwrap().offsets[index];
    let mut info = Pstates20Info::zeroed();
    info.version = unsafe { (*output).version };
    info.flags = 1;
    info.num_pstates = 1;
    info.num_clocks = 2;
    info.pstates[0].flags = 1;
    for (index, domain) in [ffi::CLOCK_GRAPHICS, ffi::CLOCK_MEMORY]
        .into_iter()
        .enumerate()
    {
        let entry = &mut info.pstates[0].clocks[index];
        entry.domain_id = domain;
        entry.flags = 1;
        entry.freq_delta_khz = ffi::ParamDelta {
            value: values[index],
            min: -1_000_000,
            max: 1_000_000,
        };
    }
    unsafe {
        *output = info;
    }
    0
}
unsafe extern "system" fn set_pstates(handle: *mut c_void, input: *mut Pstates20Info) -> i32 {
    let index = (handle as usize).wrapping_sub(1);
    if index >= 2 {
        return -5;
    }
    let request = unsafe { &*input };
    if request.num_clocks != 1 || request.num_pstates != 1 {
        return -5;
    }
    let entry = request.pstates[0].clocks[0];
    let domain = match entry.domain_id {
        ffi::CLOCK_GRAPHICS => 0,
        ffi::CLOCK_MEMORY => 1,
        _ => return -5,
    };
    let mut fake = state().lock().unwrap();
    fake.offsets[index][domain] = entry.freq_delta_khz.value;
    fake.offset_writes.push((
        index,
        request.version,
        entry.domain_id,
        entry.freq_delta_khz.value,
    ));
    0
}
unsafe extern "system" fn pci(
    handle: *mut c_void,
    device: *mut u32,
    subsystem: *mut u32,
    revision: *mut u32,
    extended: *mut u32,
) -> i32 {
    unsafe {
        *device = 0x1234_10de;
        *subsystem = 3;
        *revision = 4;
        *extended = 5;
    }
    if handle.is_null() { -5 } else { 0 }
}
unsafe extern "system" fn bus(handle: *mut c_void, value: *mut u32) -> i32 {
    unsafe {
        *value = handle as usize as u32;
    }
    0
}
unsafe extern "system" fn slot(_: *mut c_void, value: *mut u32) -> i32 {
    unsafe {
        *value = 1;
    }
    0
}
unsafe extern "system" fn get_locks(_: *mut c_void, output: *mut clock::LockTable) -> i32 {
    let mut fake = state().lock().unwrap();
    fake.get_version = unsafe { (*output).version };
    if fake.status != 0 {
        return fake.status;
    }
    unsafe {
        *output = fake.table;
    }
    0
}
unsafe extern "system" fn set_locks(handle: *mut c_void, input: *mut clock::LockTable) -> i32 {
    let mut fake = state().lock().unwrap();
    if fake.status != 0 {
        return fake.status;
    }
    let value = unsafe { *input };
    fake.writes.push((handle as usize, value));
    if let Ok(owned) = value.owned() {
        if let Ok(updated) = fake.table.with_owned(owned) {
            fake.table = updated;
        }
    }
    0
}
unsafe extern "system" fn frequencies(_: *mut c_void, input: *mut clock::Frequencies) -> i32 {
    let mut fake = state().lock().unwrap();
    fake.frequency_input = unsafe { ((*input).version, (*input).flags) };
    if fake.status != 0 {
        return fake.status;
    }
    unsafe {
        (*input).domains[0] = clock::ClockDomain {
            flags: 1,
            frequency_khz: 1_350_999,
        };
    }
    0
}

fn session() -> NvApiSession {
    let api = NvApi {
        module: std::ptr::null_mut(),
        query,
        initialize: init,
        unload,
        get_error_message: error_message,
        enum_physical_gpus: enumerate,
        get_full_name: name,
        get_pstates20: get_pstates,
        set_pstates20: set_pstates,
    };
    NvApiSession {
        api: Arc::new(api),
        handle: 1usize as *mut c_void,
        name: None,
    }
}

#[test]
fn fake_query_dispatch_reads_exact_device_ids_versions_and_base_clock() {
    let _serial = SERIAL.lock().unwrap();
    *state().lock().unwrap() = Fake::default();
    let io = session();
    let ids = io.devices().unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0].bus, 1);
    assert_eq!(ids[1].bus, 2);
    assert_eq!(io.primary().unwrap(), ids[0]);
    assert_eq!(io.base_clock(&ids[0]).unwrap(), 1350);
    assert_eq!(state().lock().unwrap().frequency_input, (0x30108, 1));
    assert_eq!(
        io.locks(&ids[0]).unwrap().owned().unwrap()[0].value,
        900_000
    );
    assert_eq!(state().lock().unwrap().get_version, 0x2030c);
}

#[test]
fn fake_set_receives_the_original_packet_and_preserves_other_domains() {
    let _serial = SERIAL.lock().unwrap();
    *state().lock().unwrap() = Fake::default();
    let io = session();
    let id = io.devices().unwrap()[0];
    let before = io.locks(&id).unwrap();
    let target = before.original_target(1700).unwrap();
    io.write_locks(&id, &target).unwrap();
    assert_eq!(
        io.locks(&id).unwrap().owned().unwrap(),
        target.owned().unwrap()
    );
    let fake = state().lock().unwrap();
    assert_eq!(fake.writes, vec![(1, target)]);
    assert_eq!(target.count, 2);
    assert_eq!(fake.table.entries[2], before.entries[2]);
    assert_eq!(fake.table.owned().unwrap()[0].value, 1_700_000);
}

#[test]
fn second_gpu_offset_uses_its_handle_and_a_single_domain_pstates_request() {
    let _serial = SERIAL.lock().unwrap();
    *state().lock().unwrap() = Fake::default();
    let io = session();
    let ids = io.devices().unwrap();
    let before = io.offsets(&ids[1]).unwrap();
    assert_eq!(before.core.delta_khz, -5_000);
    assert_eq!(before.memory.delta_khz, 30_000);
    io.write_offsets(&ids[1], None, Some(75_000)).unwrap();
    let after = io.offsets(&ids[1]).unwrap();
    assert_eq!(after.core.delta_khz, -5_000);
    assert_eq!(after.memory.delta_khz, 75_000);
    assert_eq!(
        state().lock().unwrap().offset_writes,
        [(1, ffi::VERSION_V1, ffi::CLOCK_MEMORY, 75_000)]
    );
}

#[test]
fn missing_entrypoint_driver_error_and_device_replacement_never_report_success() {
    let _serial = SERIAL.lock().unwrap();
    *state().lock().unwrap() = Fake::default();
    let io = session();
    let id = io.devices().unwrap()[0];
    state().lock().unwrap().status = -3;
    assert!(
        io.locks(&id)
            .unwrap_err()
            .to_string()
            .contains("NVAPI_NO_IMPLEMENTATION")
    );
    state().lock().unwrap().status = 0;
    let target = io.locks(&id).unwrap().original_target(500).unwrap();
    state().lock().unwrap().missing = Some(clock::SET_LOCKS);
    assert!(
        io.write_locks(&id, &target)
            .unwrap_err()
            .to_string()
            .contains("0x39442CFB")
    );
    assert!(state().lock().unwrap().writes.is_empty());
    state().lock().unwrap().missing = None;
    state().lock().unwrap().reverse = true;
    assert_ne!(io.primary().unwrap(), io.devices().unwrap()[0]);
}
