use super::Controller;
use crate::hal::{
    Hal,
    oem_dchu::Dchu,
    power_plan::{BatteryLevel, PowerAccess, SCHEMES},
};
use anyhow::{Result, bail};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

struct Memory(Mutex<State>);
struct State {
    plans: BTreeMap<u128, [[u32; 2]; 3]>,
    cache: [u8; 3],
    writes: Vec<String>,
    unexpected: Vec<String>,
}
thread_local! { static CURRENT: RefCell<Option<Arc<Memory>>> = const { RefCell::new(None) }; }
fn index(level: BatteryLevel) -> usize {
    match level {
        BatteryLevel::Low => 0,
        BatteryLevel::Critical => 1,
        BatteryLevel::Reserve => 2,
    }
}
impl PowerAccess for Memory {
    fn lid_action(&self, _: u128) -> Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入合盖计划动作")
    }
    fn battery_action(&self, _: u128, _: crate::hal::power_plan::BatteryAction) -> Result<u32> {
        bail!("此隔离提供者未注入电池计划动作")
    }
    fn active(&self) -> Result<u128> {
        Ok(SCHEMES[0])
    }
    fn schemes(&self) -> Result<Vec<u128>> {
        Ok(self.0.lock().unwrap().plans.keys().copied().collect())
    }
    fn battery_level(&self, scheme: u128, level: BatteryLevel) -> Result<[u32; 2]> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .plans
            .get(&scheme)
            .ok_or_else(|| anyhow::anyhow!("隔离计划不存在"))?[index(level)])
    }
    fn set_battery_level(
        &self,
        scheme: u128,
        level: BatteryLevel,
        ac: bool,
        value: u32,
    ) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        state
            .plans
            .get_mut(&scheme)
            .ok_or_else(|| anyhow::anyhow!("隔离计划不存在"))?[index(level)][usize::from(!ac)] =
            value;
        state
            .writes
            .push(format!("plan:{scheme}:{level:?}:{ac}:{value}"));
        Ok(())
    }
    fn activate(&self, scheme: u128) -> Result<()> {
        if scheme != SCHEMES[0] {
            bail!("不能切换隔离活动计划");
        }
        self.0.lock().unwrap().writes.push("refresh".into());
        Ok(())
    }
    fn usb(&self, _: u128) -> Result<[u32; 2]> {
        bail!("未注入USB")
    }
    fn set_usb(&self, _: u128, _: bool, _: u32) -> Result<()> {
        bail!("未注入USB")
    }
    fn aspm(&self, _: u128) -> Result<[u32; 2]> {
        bail!("未注入ASPM")
    }
    fn set_aspm(&self, _: u128, _: bool, _: u32) -> Result<()> {
        bail!("未注入ASPM")
    }
    fn brightness(&self, _: u128) -> Result<[u32; 2]> {
        bail!("未注入屏幕亮度")
    }
    fn set_brightness(&self, _: u128, _: bool, _: u32) -> Result<()> {
        bail!("未注入屏幕亮度")
    }
}
unsafe extern "system" fn packet(command: i32, _: *mut u8) -> i32 {
    CURRENT.with_borrow(|slot| {
        if let Some(memory) = slot {
            memory
                .0
                .lock()
                .unwrap()
                .unexpected
                .push(format!("packet:{command}"));
        }
    });
    0
}
unsafe extern "system" fn send(command: i32, _: *const u8, _: i32) -> i32 {
    CURRENT.with_borrow(|slot| {
        if let Some(memory) = slot {
            memory
                .0
                .lock()
                .unwrap()
                .unexpected
                .push(format!("send:{command}"));
        }
    });
    0
}
unsafe extern "system" fn read(page: i32, offset: i32, length: i32, output: *mut u8) -> i32 {
    CURRENT.with_borrow(|slot| {
        let Some(memory) = slot else {
            return 0;
        };
        let mut state = memory.0.lock().unwrap();
        if page != 1 || offset != 47 || length != 3 {
            state
                .unexpected
                .push(format!("read:{page}:{offset}:{length}"));
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(state.cache.as_ptr(), output, 3);
        }
        4096
    })
}
unsafe extern "system" fn write(page: i32, offset: i32, length: i32, input: *const u8) -> i32 {
    CURRENT.with_borrow(|slot| {
        let Some(memory) = slot else {
            return 0;
        };
        let mut state = memory.0.lock().unwrap();
        if page != 1 || !(47..=49).contains(&offset) || length != 1 {
            state
                .unexpected
                .push(format!("write:{page}:{offset}:{length}"));
            return 0;
        }
        let value = unsafe { *input };
        state.cache[(offset - 47) as usize] = value;
        state.writes.push(format!("cache:{offset}:{value}"));
        1
    })
}
#[test]
fn production_controller_uses_only_injected_power_and_exact_oem_cache_fields() {
    let temp = tempfile::tempdir().unwrap();
    let initial = [[10, 11], [5, 7], [4, 6]];
    let memory = Arc::new(Memory(Mutex::new(State {
        plans: BTreeMap::from([(SCHEMES[0], initial)]),
        cache: [0; 3],
        writes: vec![],
        unexpected: vec![],
    })));
    CURRENT.set(Some(memory.clone()));
    let mut hal = Hal::test_instance(Dchu::test_io(packet, read, send, write), None);
    hal.system_power = memory.clone();
    let controller = Controller::new(temp.path().join("recovery.json"));
    let identity = controller.read(&hal).unwrap().identity.unwrap();
    assert!(memory.0.lock().unwrap().writes.is_empty());
    controller.run(&hal, &identity, 209, || Ok(())).unwrap();
    let profile = controller.read(&hal).unwrap();
    assert_eq!(profile.identity.as_deref(), Some(identity.as_str()));
    assert!(controller.restore(&hal, "stale-preview").is_err());
    controller
        .restore(&hal, profile.restore_revision.as_deref().unwrap())
        .unwrap();
    let state = memory.0.lock().unwrap();
    assert_eq!(state.plans[&SCHEMES[0]], initial);
    assert_eq!(state.cache, [0; 3]);
    assert!(state.unexpected.is_empty());
    assert_eq!(
        state
            .writes
            .iter()
            .filter(|event| event.starts_with("cache:"))
            .count(),
        6
    );
    drop(state);
    controller.run(&hal, &identity, 209, || Ok(())).unwrap();
    controller.run(&hal, &identity, 210, || Ok(())).unwrap();
    let state = memory.0.lock().unwrap();
    assert_eq!(state.plans[&SCHEMES[0]], initial);
    assert_eq!(state.cache, [0; 3]);
    assert!(state.unexpected.is_empty());
}
