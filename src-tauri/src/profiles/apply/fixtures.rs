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
        firmware::DirectControl,
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
    pub fan_readbacks: std::collections::VecDeque<u8>,
    pub fan_readbacks_on_preset: std::collections::VecDeque<u8>,
    pub fan_readbacks_on_commands: std::collections::VecDeque<std::collections::VecDeque<u8>>,
    pub fan_mode: u8,
    pub offset_raw: u8,
    pub fan_curves: [[u8; 14]; 3],
    pub ignored_fan_writes: usize,
    pub fail_curve_commands: usize,
    pub fail_rpm_write: bool,
    direct_control: [DirectControl; 2],
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
fn direct_control(_: &Dchu) -> anyhow::Result<[DirectControl; 2]> {
    Ok(device().0.lock().unwrap().direct_control)
}
#[path = "fixtures_device.rs"]
mod device_io;
pub use device_io::io;

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
            fan_readbacks: Default::default(),
            fan_readbacks_on_preset: Default::default(),
            fan_readbacks_on_commands: Default::default(),
            fan_mode: 0,
            offset_raw: 0,
            fan_curves: device_io::FACTORY_CURVES,
            ignored_fan_writes: 0,
            fail_curve_commands: 0,
            fail_rpm_write: false,
            direct_control: [DirectControl::Firmware; 2],
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
    pub(super) fn restore_fan(&self, target: crate::fanctl::RestoreTarget) -> anyhow::Result<()> {
        let expected = crate::fanctl::read_restore_view(&self.hal)?.restore_revision;
        crate::fanctl::restore(&self.hal, target, &expected)
    }

    pub fn with_energy(mut self, value: u8) -> Self {
        self.hal.reviewed_dsdt = true;
        let mut state = self.device.0.lock().unwrap();
        let packet = energy_packet::software(value);
        state.bios[..13].copy_from_slice(&packet[..13]);
        drop(state);
        self
    }
    pub fn new() -> Self {
        let serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let device = Device::initial();
        *ACTIVE.lock().unwrap() = Some(device.clone());
        let mut hal = Hal::test_instance(io(), None).with_test_msr(device.clone());
        hal.test_fan_control = Some(direct_control);
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
