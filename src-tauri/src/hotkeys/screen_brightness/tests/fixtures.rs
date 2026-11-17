use super::super::{Controller, model::Snapshot, transaction::Io};
use crate::hal::{Hal, oem_dchu::Dchu, power_plan::PowerAccess, screen_brightness::Reading};
use anyhow::{Result, bail};
use ccc_core::display_brightness::Targets;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

pub struct Memory(pub Mutex<Data>);
pub struct Data {
    pub snapshot: Snapshot,
    pub plans: BTreeMap<u128, [u32; 2]>,
    pub writes: Vec<(u128, bool, u32)>,
    pub activations: Vec<u128>,
    pub fail_writes: Vec<usize>,
    pub ignore_writes: Vec<usize>,
    pub fail_activations: Vec<usize>,
    pub fail_packet: bool,
    pub fail_panel: bool,
    pub panel_reads: usize,
    pub fail_levels: bool,
    pub writable: bool,
    pub unexpected: Vec<i32>,
}
thread_local! { static CURRENT: RefCell<Option<Arc<Memory>>> = const { RefCell::new(None) }; }

pub fn setup() -> (Controller, Hal, Arc<Memory>) {
    let snapshot = Snapshot {
        reading: Reading {
            panel: "isolated-panel".into(),
            current: 80,
            on_ac: false,
        },
        plan: 1,
        levels: [75, 65],
        targets: Targets([40; 11]),
    };
    let memory = Arc::new(Memory(Mutex::new(Data {
        plans: BTreeMap::from([(1, snapshot.levels), (2, [55, 45])]),
        snapshot,
        writes: vec![],
        activations: vec![],
        fail_writes: vec![],
        ignore_writes: vec![],
        fail_activations: vec![],
        fail_packet: false,
        fail_panel: false,
        panel_reads: 0,
        fail_levels: false,
        writable: true,
        unexpected: vec![],
    })));
    CURRENT.set(Some(memory.clone()));
    let mut hal = Hal::test_instance(
        Dchu::test_io(packet, read_settings, send, write_settings),
        None,
    );
    hal.system_power = memory.clone();
    let report = memory.clone();
    hal.test_screen_brightness = Some(Arc::new(move || report.panel()));
    (Controller::default(), hal, memory)
}
unsafe extern "system" fn packet(command: i32, output: *mut u8) -> i32 {
    CURRENT.with_borrow(|slot| {
        let Some(memory) = slot else {
            return 0;
        };
        let mut data = memory.0.lock().unwrap();
        if command != 17 {
            data.unexpected.push(command);
            return 0;
        }
        if data.fail_packet {
            return 0;
        }
        let mut bytes = [0; 256];
        bytes[210..221].copy_from_slice(&data.snapshot.targets.0);
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
            output.add(256).write(bytes[0]);
        }
        command
    })
}
fn unexpected(command: i32) -> i32 {
    CURRENT.with_borrow(|slot| {
        if let Some(memory) = slot {
            memory.0.lock().unwrap().unexpected.push(command);
        }
    });
    0
}
unsafe extern "system" fn send(command: i32, _: *const u8, _: i32) -> i32 {
    unexpected(command)
}
unsafe extern "system" fn read_settings(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
    unexpected(-1)
}
unsafe extern "system" fn write_settings(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    unexpected(-2)
}

impl Io for Memory {
    fn snapshot(&self) -> Result<Snapshot> {
        let data = self.0.lock().unwrap();
        if data.fail_packet || data.fail_panel || data.fail_levels {
            bail!("隔离报告读取失败");
        }
        let mut snapshot = data.snapshot.clone();
        snapshot.levels = *data
            .plans
            .get(&snapshot.plan)
            .ok_or_else(|| anyhow::anyhow!("计划缺失"))?;
        Ok(snapshot)
    }
    fn panel(&self) -> Result<Reading> {
        let mut data = self.0.lock().unwrap();
        data.panel_reads += 1;
        if data.fail_panel {
            bail!("隔离面板报告失败");
        }
        Ok(data.snapshot.reading.clone())
    }
    fn active(&self) -> Result<u128> {
        Ok(self.0.lock().unwrap().snapshot.plan)
    }
    fn levels(&self, plan: u128) -> Result<[u32; 2]> {
        let data = self.0.lock().unwrap();
        if data.fail_levels {
            bail!("隔离计划读取失败");
        }
        data.plans
            .get(&plan)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("隔离计划缺失"))
    }
    fn write(&self, plan: u128, ac: bool, value: u32) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.writes.push((plan, ac, value));
        let attempt = data.writes.len();
        if !data.ignore_writes.contains(&attempt) {
            data.plans.get_mut(&plan).unwrap()[usize::from(!ac)] = value;
        }
        if data.fail_writes.contains(&attempt) {
            bail!("隔离计划部分写入后失败{attempt}");
        }
        Ok(())
    }
    fn activate(&self, plan: u128) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.activations.push(plan);
        if data.fail_activations.contains(&data.activations.len()) {
            bail!("隔离计划刷新失败");
        }
        data.snapshot.plan = plan;
        Ok(())
    }
    fn ensure_writable(&self) -> Result<()> {
        if !self.0.lock().unwrap().writable {
            bail!("隔离校准占用计划");
        }
        Ok(())
    }
}
impl PowerAccess for Memory {
    fn lid_action(&self, _: u128) -> Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入合盖计划动作")
    }
    fn battery_action(&self, _: u128, _: crate::hal::power_plan::BatteryAction) -> Result<u32> {
        bail!("此隔离提供者未注入电池计划动作")
    }
    fn battery_level(&self, _: u128, _: crate::hal::power_plan::BatteryLevel) -> Result<[u32; 2]> {
        bail!("此隔离提供者未注入电池计划阈值")
    }
    fn set_battery_level(
        &self,
        _: u128,
        _: crate::hal::power_plan::BatteryLevel,
        _: bool,
        _: u32,
    ) -> Result<()> {
        bail!("此隔离提供者未注入电池计划阈值")
    }
    fn active(&self) -> Result<u128> {
        Io::active(self)
    }
    fn schemes(&self) -> Result<Vec<u128>> {
        Ok(self.0.lock().unwrap().plans.keys().copied().collect())
    }
    fn brightness(&self, plan: u128) -> Result<[u32; 2]> {
        self.levels(plan)
    }
    fn set_brightness(&self, plan: u128, ac: bool, value: u32) -> Result<()> {
        self.write(plan, ac, value)
    }
    fn activate(&self, plan: u128) -> Result<()> {
        Io::activate(self, plan)
    }
    fn usb(&self, _: u128) -> Result<[u32; 2]> {
        bail!("不允许访问USB字段")
    }
    fn set_usb(&self, _: u128, _: bool, _: u32) -> Result<()> {
        bail!("不允许写入USB字段")
    }
    fn aspm(&self, _: u128) -> Result<[u32; 2]> {
        bail!("不允许访问ASPM字段")
    }
    fn set_aspm(&self, _: u128, _: bool, _: u32) -> Result<()> {
        bail!("不允许写入ASPM字段")
    }
}
pub fn identity(controller: &Controller, hal: &Hal) -> String {
    let profile = controller.read(hal).unwrap();
    profile
        .identity
        .clone()
        .unwrap_or_else(|| panic!("{profile:?}"))
}
pub fn no_native(memory: &Memory) {
    assert!(memory.0.lock().unwrap().unexpected.is_empty());
}
