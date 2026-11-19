#[path = "battery_fixture.rs"]
pub mod battery_fixture;
#[path = "fixtures_devices.rs"]
#[cfg(not(control_sim))]
mod devices;
#[path = "fixtures_energy.rs"]
mod energy_packet;
#[path = "microphone_fixture.rs"]
pub mod microphone_fixture;
#[path = "fixtures_mode_source.rs"]
mod mode_source;
#[path = "fixtures_power.rs"]
mod power;
#[path = "touchpad_fixture.rs"]
pub mod touchpad_fixture;
#[path = "fixtures_usb.rs"]
mod usb;
use crate::{
    gpuctl::GpuController,
    hal::{
        Hal,
        nvapi::{
            pstates::{DomainReading, ParsedPstates},
            transaction::PstatesIo,
        },
        oem_dchu::Dchu,
    },
};
use ccc_core::msr::{self, MsrAccess};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct State {
    pub bios: [u8; 256],
    pub policy_tables: std::collections::BTreeMap<u8, [u8; 256]>,
    pub active_plan: u128,
    pub plan_values: std::collections::BTreeMap<u128, [u32; 2]>,
    pub usb_values: std::collections::BTreeMap<u128, [u32; 2]>,
    pub usb_writes: usize,
    pub fail_usb_at: Vec<usize>,
    pub pd_level: [u8; 2],
    pub fail_pd: usize,
    pub fail_plan: bool,
    pub fail_default: bool,
    pub fail_mode_save: bool,
    pub fail_option_save: Option<(u8, usize)>,
    pub pages: [[u8; 256]; 16],
    pub power: u64,
    pub voltage: [i16; 3],
    pub response: u64,
    pub events: Vec<String>,
    pub ignore_gt_target: bool,
    pub fail_restore_pl: bool,
    pub fail_gpu: bool,
    pub fail_saved: bool,
    pub fail_mode: bool,
    pub fail_energy: Option<u8>,
    pub ignore_energy_write: bool,
    pub fail_fan_mode: bool,
    pub fan_command: Option<u8>,
    pub fan_count: u8,
    pub cpu_temperature: u8,
    pub gpu_khz: i32,
    pub dust_running: bool,
    pub fail_dust_commands: usize,
    pub fail_dust_save: bool,
    pub keyboard_live: [u8; 5],
    pub keyboard_boot_supported: bool,
    pub fail_keyboard_save: bool,
    pub fail_keyboard_send: usize,
    pub fail_lighting_words: usize,
    pub fail_page2_at: Option<usize>,
}
pub struct Device(pub Mutex<State>);
static ACTIVE: Mutex<Option<Arc<Device>>> = Mutex::new(None);
static SERIAL: Mutex<()> = Mutex::new(());

fn device() -> Arc<Device> {
    ACTIVE.lock().unwrap().as_ref().unwrap().clone()
}
unsafe extern "system" fn buffer(command: i32, out: *mut u8) -> i32 {
    let mut packet = [0; 257];
    match command {
        12 => {
            packet[18] = device().0.lock().unwrap().cpu_temperature;
            packet[16] = 76;
            packet[19] = 76;
        }
        13 => {
            let d = device();
            let state = d.0.lock().unwrap();
            packet[15] = state.pages[2][4];
            packet[43] = u8::from(state.keyboard_boot_supported);
            packet[12] = state.fan_count;
            packet[50] = (1 << state.fan_count) - 1;
            for at in [16, 24] {
                packet[at..at + 6].copy_from_slice(&[40, 51, 60, 102, 80, 204]);
            }
        }
        17 | 18 => (),
        _ => return 0,
    }
    unsafe {
        std::ptr::copy_nonoverlapping(packet.as_ptr(), out, packet.len());
    }
    command
}
unsafe extern "system" fn read(page: i32, offset: i32, length: i32, out: *mut u8) -> i32 {
    let d = device();
    let state = d.0.lock().unwrap();
    unsafe {
        std::ptr::copy_nonoverlapping(
            state.pages[page as usize][offset as usize..].as_ptr(),
            out,
            length as usize,
        );
    }
    4096
}
unsafe extern "system" fn write(page: i32, offset: i32, length: i32, bytes: *const u8) -> i32 {
    let d = device();
    let mut state = d.0.lock().unwrap();
    let bytes = unsafe { std::slice::from_raw_parts(bytes, length as usize) };
    state.events.push(format!("PAGE{page}:{offset}:{bytes:?}"));
    state.pages[page as usize][offset as usize..offset as usize + bytes.len()]
        .copy_from_slice(bytes);
    if page == 1 && offset == 1 && state.fail_mode_save {
        state.fail_mode_save = false;
        return 0;
    }
    if state.fail_option_save == Some((page as u8, offset as usize)) {
        state.fail_option_save = None;
        return 0;
    }
    if page == 6 && offset == 69 && state.fail_saved {
        state.fail_saved = false;
        return 0;
    }
    if page == 4 && offset == 81 && state.fail_dust_save {
        state.fail_dust_save = false;
        return 0;
    }
    if page == 1 && offset == 36 && state.fail_keyboard_save {
        state.fail_keyboard_save = false;
        return 0;
    }
    if page == 2 && state.fail_page2_at == Some(offset as usize) {
        state.fail_page2_at = None;
        return 0;
    }
    1
}
unsafe extern "system" fn set(command: i32, bytes: *const u8, length: i32) -> i32 {
    let d = device();
    let mut state = d.0.lock().unwrap();
    let bytes = unsafe { std::slice::from_raw_parts(bytes, length as usize) };
    if command == 79 {
        if !energy_packet::set(&mut state, bytes[0]) {
            return 0;
        }
    } else if matches!(command, 39 | 103) {
        let word = u32::from_le_bytes(bytes[..4].try_into().unwrap());
        state.events.push(format!("LIGHT:{command}:{word:08x}"));
        if state.fail_lighting_words != 0 {
            state.fail_lighting_words -= 1;
            return 0;
        }
    } else if command == 121 && matches!(bytes[3], 40 | 41) {
        let sub = bytes[3];
        let value = u32::from_le_bytes(bytes[..4].try_into().unwrap()) & 0xffffff;
        state.events.push(format!("DUST:{sub}:{value}"));
        if state.fail_dust_commands != 0 {
            state.fail_dust_commands -= 1;
            return 0;
        }
    } else if command == 121 && bytes[3] == 25 {
        let mode = bytes[0];
        state.events.push(format!("MODE:{mode}"));
        state.power = ((1040 | 0x8000) << 32) | (720 | 0x8000);
        state.voltage = [-20, -21, 0];
        if state.fail_mode {
            return 0;
        }
    } else if command == 121 && bytes[3] == 34 {
        state.events.push("FAN_RESET".into());
        if state.fail_default {
            state.fail_default = false;
            return 0;
        }
    } else if command == 121 && bytes[3] == 1 && state.fail_fan_mode {
        state.fail_fan_mode = false;
        state.events.push("FAN_MODE_FAILURE".into());
        return 0;
    } else {
        if command == 121 && bytes[3] == 1 {
            state.fan_command = Some(bytes[0]);
        }
        state.events.push(format!("DCHU:{command}"));
    }
    command
}
pub fn io() -> Dchu {
    Dchu::test_io(buffer, read, set, write).with_test_exchange(exchange)
}

unsafe extern "system" fn exchange(command: i32, input: *const u8, _: i32, out: *mut u8) -> i32 {
    let selector = unsafe { *input };
    let d = device();
    let mut state = d.0.lock().unwrap();
    if matches!(selector, 24 | 25) {
        if selector == 25 {
            let values: [u8; 5] = unsafe {
                std::slice::from_raw_parts(input.add(1), 5)
                    .try_into()
                    .unwrap()
            };
            state.keyboard_live = values;
            state.events.push(format!("KEYBOARD:{values:?}"));
            if state.fail_keyboard_send != 0 {
                state.fail_keyboard_send -= 1;
                return 0;
            }
        }
        unsafe {
            std::ptr::write_bytes(out, 0, 257);
            std::ptr::copy_nonoverlapping(state.keyboard_live.as_ptr(), out, 5);
            *out.add(256) = state.keyboard_live[0];
        }
        return command;
    }
    if selector == 1 && unsafe { *input.add(1) } == 163 {
        unsafe {
            std::ptr::write_bytes(out, 0, 257);
            *out.add(1) = u8::from(state.dust_running);
        }
        return command;
    }
    if selector == 54 || selector == 55 {
        if selector == 55 {
            state.pd_level[0] = unsafe { *input.add(1) };
            let level = state.pd_level[0];
            state.events.push(format!("PD:{level}"));
            if state.fail_pd != 0 {
                state.fail_pd -= 1;
                return 0;
            }
        }
        unsafe {
            std::ptr::write_bytes(out, 0, 257);
            std::ptr::copy_nonoverlapping(state.pd_level.as_ptr(), out, 2);
            *out.add(256) = state.pd_level[0];
        }
        return command;
    }
    let table = if selector == 8 {
        Some(&state.bios)
    } else {
        state.policy_tables.get(&selector)
    };
    let Some(table) = table else {
        return 0;
    };
    unsafe {
        std::ptr::copy_nonoverlapping(table.as_ptr(), out, 256);
        *out.add(256) = table[0];
    }
    command
}

impl MsrAccess for Device {
    fn read(&self, address: u32) -> Result<u64, String> {
        let state = self.0.lock().unwrap();
        Ok(match address {
            msr::MSR_RAPL_POWER_UNIT => 3,
            msr::MSR_PKG_POWER_LIMIT => state.power,
            _ => state.response,
        })
    }
    fn write(&self, address: u32, value: u64) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        if address == msr::MSR_PKG_POWER_LIMIT {
            state.events.push(format!("PL:{value:#x}"));
            if state.fail_restore_pl && value & 0x7fff == 529 {
                return Err("injected PL restore failure".into());
            }
            state.power = value;
        } else {
            let plane = ((value >> 40) & 15) as usize;
            let units = msr::decode_offset(value);
            if (value >> 32) & 0xff == 0x11 {
                state.events.push(format!("UV:{plane}:{units}"));
                if !(state.ignore_gt_target && plane == 1 && units == -30) {
                    state.voltage[plane] = units;
                }
            }
            state.response = ((state.voltage[plane] as u16 & 0x7ff) as u64) << 21;
        }
        Ok(())
    }
}
struct Gpu(
    Arc<Device>,
    Arc<crate::gpu_power::test_fixture::EnvironmentFixture>,
);
impl Gpu {
    fn ensure_connected(&self) -> anyhow::Result<()> {
        let data = self.1.io.0.lock().unwrap();
        if data.supported && !data.device.enabled {
            anyhow::bail!("隔离GPU已关闭");
        }
        Ok(())
    }
}
impl PstatesIo for Gpu {
    fn read(&self) -> anyhow::Result<ParsedPstates> {
        self.ensure_connected()?;
        let state = self.0.0.lock().unwrap();
        let domain = |delta| DomainReading {
            present: true,
            editable: true,
            delta_khz: delta,
            min_khz: -500_000,
            max_khz: 900_000,
        };
        Ok(ParsedPstates {
            editable: true,
            p0_present: true,
            core: domain(state.gpu_khz),
            memory: domain(0),
            count_clamped: false,
        })
    }
    fn write(&self, core: Option<i32>, _: Option<i32>) -> anyhow::Result<()> {
        self.ensure_connected()?;
        let mut state = self.0.0.lock().unwrap();
        if let Some(value) = core {
            state.events.push(format!("GPU:{value}"));
            state.gpu_khz = value;
            if state.fail_gpu && value == 60_000 {
                anyhow::bail!("injected GPU target failure");
            }
        }
        Ok(())
    }
}

pub struct Fixture {
    pub hal: Hal,
    pub gpu: GpuController,
    pub light_bar: crate::lighting::lightbar::Controller,
    pub multi_zone: crate::lighting::multizone::Controller,
    pub dust: crate::fanctl::dust::Controller,
    pub keyboard: crate::keyboard::Controller,
    pub microphones: crate::microphone::Controller,
    pub touchpad: crate::touchpad::Controller,
    pub touchpad_io: Arc<touchpad_fixture::Memory>,
    pub microphone_io: Arc<microphone_fixture::Memory>,
    pub lighting: crate::lighting::LightingController,
    pub lighting_options: crate::lighting::options::Controller,
    pub perkey: crate::lighting::perkey::Controller,
    pub keymap: crate::keymap::Controller,
    pub power_delivery: crate::platform::Controller,
    pub usb: crate::platform::usb::Controller,
    pub battery: battery_fixture::Fixture,
    pub lan: crate::platform::lan::test_fixture::Fixture,
    pub display: crate::display::test_fixture::Fixture,
    pub gpu_power: crate::gpu_power::Controller,
    pub graphics_mode: crate::graphics_mode::test_fixture::Fixture,
    pub gpu_environment: Arc<crate::gpu_power::test_fixture::EnvironmentFixture>,
    _keymap_dir: tempfile::TempDir,
    pub device: Arc<Device>,
    _serial: MutexGuard<'static, ()>,
}
impl Device {
    fn initial() -> Arc<Self> {
        let mut pages = [[0; 256]; 16];
        pages[1][1] = 3;
        let mut bios = energy_packet::hardware();
        bios[17] = 15;
        pages[6][33..37].copy_from_slice(&65i32.to_le_bytes());
        pages[6][37..41].copy_from_slice(&100i32.to_le_bytes());
        Arc::new(Self(Mutex::new(State {
            bios,
            policy_tables: Default::default(),
            usb_values: crate::hal::power_plan::SCHEMES
                .into_iter()
                .map(|id| (id, [0, 1]))
                .collect(),
            usb_writes: 0,
            fail_usb_at: vec![],
            pd_level: [1, 0],
            fail_pd: 0,
            active_plan: crate::hal::power_plan::SCHEMES[0],
            plan_values: crate::hal::power_plan::SCHEMES
                .into_iter()
                .map(|id| (id, [0, 1]))
                .collect(),
            fail_plan: false,
            fail_default: false,
            fail_mode_save: false,
            fail_option_save: None,
            pages,
            power: ((801 | 0x8000) << 32) | (529 | 0x8000),
            voltage: [-51, -26, 0],
            response: 0,
            events: vec![],
            ignore_gt_target: false,
            fail_restore_pl: false,
            fail_gpu: false,
            fail_saved: false,
            fail_mode: false,
            fail_energy: None,
            ignore_energy_write: false,
            fail_fan_mode: false,
            fan_command: None,
            fan_count: 2,
            cpu_temperature: 50,
            gpu_khz: 135_000,
            dust_running: false,
            fail_dust_commands: 0,
            fail_dust_save: false,
            keyboard_live: [0; 5],
            keyboard_boot_supported: false,
            fail_keyboard_save: false,
            fail_keyboard_send: 0,
            fail_lighting_words: 0,
            fail_page2_at: None,
        })))
    }
}
impl Fixture {
    pub fn with_energy(mut self, value: u8) -> Self {
        self.hal.reviewed_dsdt = true;
        let mut state = self.device.0.lock().unwrap();
        let packet = energy_packet::software(value);
        state.bios[..13].copy_from_slice(&packet[..13]);
        drop(state);
        self
    }
    pub fn new() -> Self {
        let serial = SERIAL.lock().unwrap();
        let device = Device::initial();
        *ACTIVE.lock().unwrap() = Some(device.clone());
        let mut hal = Hal::test_instance(io(), None).with_test_msr(device.clone());
        hal.system_power = device.clone();
        let gpu_environment = crate::gpu_power::test_fixture::EnvironmentFixture::new();
        gpu_environment.io.0.lock().unwrap().supported = false;
        let gpu = GpuController::test_with_session(Arc::new(Gpu(
            device.clone(),
            gpu_environment.clone(),
        )));
        let keymap_dir = tempfile::tempdir().unwrap();
        let keymap = crate::keymap::Controller::new(keymap_dir.path().join("keymap-recovery.json"));
        let gpu_power = crate::gpu_power::Controller::with_environment(gpu_environment.clone());
        let microphone_io = microphone_fixture::Memory::new(device.clone());
        let touchpad_io = touchpad_fixture::Memory::new(device.clone());
        Self {
            hal,
            gpu,
            light_bar: Default::default(),
            multi_zone: Default::default(),
            dust: Default::default(),
            keyboard: Default::default(),
            touchpad: crate::touchpad::Controller::default().with_test_io(touchpad_io.clone()),
            touchpad_io,
            microphones: crate::microphone::Controller::default()
                .with_test_io(microphone_io.clone()),
            microphone_io,
            lighting: Default::default(),
            lighting_options: Default::default(),
            perkey: Default::default(),
            keymap,
            power_delivery: Default::default(),
            usb: crate::platform::usb::Controller::test(device.clone()),
            battery: battery_fixture::Fixture::new(),
            lan: crate::platform::lan::test_fixture::Fixture::new(),
            display: crate::display::test_fixture::Fixture::new(),
            gpu_power,
            graphics_mode: crate::graphics_mode::test_fixture::Fixture::new(),
            gpu_environment,
            _keymap_dir: keymap_dir,
            device,
            _serial: serial,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        ACTIVE.lock().unwrap().take();
    }
}
