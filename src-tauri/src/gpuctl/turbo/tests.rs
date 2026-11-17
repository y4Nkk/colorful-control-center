use super::{
    model::*,
    source::{Source, SourceIo},
    transaction::State,
};
use crate::gpuctl::GpuCheckpoint;
use crate::hal::nvapi::{
    clock::{LockEntry, LockTable},
    pstates::{DomainReading, ParsedPstates},
    transaction::RawOffsets,
    turbo::{DeviceId, TurboIo},
};
use anyhow::{Result, bail};
use ccc_core::gpu_turbo::{Capabilities, Limits, Offsets};
use std::sync::Mutex;
#[path = "test_authorization.rs"]
mod authorization_tests;
#[path = "mode/execution_tests.rs"]
mod mode_execution_tests;

struct Memory(Mutex<Data>);
struct Data {
    devices: Vec<DeviceId>,
    locks: LockTable,
    offsets: Vec<ParsedPstates>,
    source: Source,
    writes: Vec<String>,
    write_observer: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    fail_on: Vec<usize>,
    reads: usize,
    change_mode_on_read: Option<usize>,
    lock_reads: usize,
    fail_lock_read_at: Option<usize>,
}

fn device(bus: u32) -> DeviceId {
    DeviceId {
        pci_device: 0x10de_1234,
        subsystem: 3,
        revision: 4,
        extended: 5,
        bus,
        slot: 1,
    }
}

fn domain(value: i32, min: i32, max: i32) -> DomainReading {
    DomainReading {
        present: true,
        editable: true,
        delta_khz: value,
        min_khz: min,
        max_khz: max,
    }
}

fn parsed(core: i32, memory: i32) -> ParsedPstates {
    ParsedPstates {
        editable: true,
        p0_present: true,
        count_clamped: false,
        core: domain(core, -300_000, 300_000),
        memory: domain(memory, -1_000_000, 1_000_000),
    }
}

fn fixture() -> Memory {
    let mut locks = LockTable {
        count: 3,
        ..Default::default()
    };
    locks.entries[0] = LockEntry {
        domain: 0,
        mode: 3,
        value: 900_000,
        unknown1: 42,
        ..Default::default()
    };
    locks.entries[1] = LockEntry {
        domain: 1,
        mode: 3,
        value: 100_000,
        ..Default::default()
    };
    locks.entries[2] = LockEntry {
        domain: 4,
        mode: 3,
        value: 500,
        ..Default::default()
    };
    Memory(Mutex::new(Data {
        devices: vec![device(1), device(2)],
        locks,
        offsets: vec![parsed(10_000, 20_000), parsed(-5_000, 30_000)],
        source: Source {
            capabilities: Capabilities {
                limit_clock: true,
                all_mode_oc: true,
            },
            limits: Limits {
                entertainment_mhz: 1400,
                performance_mhz: 1700,
                saving_mhz: 900,
                quiet_mhz: 700,
                thermal_mhz: 650,
            },
            saved: [
                Offsets {
                    core_mhz: 100,
                    memory_mhz: -1,
                },
                Offsets {
                    core_mhz: 1,
                    memory_mhz: 0,
                },
            ],
            mode: 2,
            on_ac: Some(true),
            supply_error: None,
        },
        writes: Vec::new(),
        write_observer: None,
        fail_on: Vec::new(),
        reads: 0,
        change_mode_on_read: None,
        lock_reads: 0,
        fail_lock_read_at: None,
    }))
}

impl Data {
    fn record(&mut self, name: String) -> Result<()> {
        if let Some(observer) = &self.write_observer {
            observer();
        }
        self.writes.push(name);
        let number = self.writes.len();
        if let Some(index) = self.fail_on.iter().position(|value| *value == number) {
            self.fail_on.remove(index);
            bail!("注入第 {number} 次写入失败");
        }
        Ok(())
    }
    fn index(&self, id: &DeviceId) -> Result<usize> {
        self.devices
            .iter()
            .position(|candidate| candidate == id)
            .ok_or_else(|| anyhow::anyhow!("设备不存在"))
    }
}

impl SourceIo for Memory {
    fn read(&self) -> Result<Source> {
        let mut data = self.0.lock().unwrap();
        data.reads += 1;
        if data.change_mode_on_read == Some(data.reads) {
            data.source.mode = 1;
        }
        Ok(data.source.clone())
    }
}

impl TurboIo for Memory {
    fn devices(&self) -> Result<Vec<DeviceId>> {
        Ok(self.0.lock().unwrap().devices.clone())
    }
    fn primary(&self) -> Result<DeviceId> {
        Ok(device(1))
    }
    fn base_clock(&self, _id: &DeviceId) -> Result<u32> {
        Ok(1350)
    }
    fn locks(&self, id: &DeviceId) -> Result<LockTable> {
        let mut data = self.0.lock().unwrap();
        if data.index(id)? != 0 {
            bail!("锁频目标不是第一张显卡");
        }
        data.lock_reads += 1;
        if data.fail_lock_read_at == Some(data.lock_reads) {
            bail!("注入锁频准备读取失败");
        }
        Ok(data.locks)
    }
    fn write_locks(&self, id: &DeviceId, table: &LockTable) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        if data.index(id)? != 0 {
            bail!("锁频目标不是第一张显卡");
        }
        data.locks = data.locks.with_owned(table.owned()?)?;
        data.record("lock".into())
    }
    fn offsets(&self, id: &DeviceId) -> Result<ParsedPstates> {
        let data = self.0.lock().unwrap();
        Ok(data.offsets[data.index(id)?])
    }
    fn write_offsets(&self, id: &DeviceId, core: Option<i32>, memory: Option<i32>) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        let index = data.index(id)?;
        if let Some(value) = core {
            data.offsets[index].core.delta_khz = value;
        }
        if let Some(value) = memory {
            data.offsets[index].memory.delta_khz = value;
        }
        data.record(format!(
            "gpu{index}:{}",
            if core.is_some() { "core" } else { "memory" }
        ))
    }
}

fn identity(memory: &Memory) -> String {
    capture(memory, memory.read().unwrap())
        .unwrap()
        .identity()
        .unwrap()
}

fn execute(
    state: &mut State,
    memory: &Memory,
    shared: &mut Option<GpuCheckpoint>,
    event: u8,
) -> Result<String> {
    let expected = identity(memory);
    state.execute(memory, memory, shared, &expected, event, || Ok(()))
}

#[test]
fn turbo_on_runs_original_order_and_restores_both_cards_and_unowned_domains() {
    let memory = fixture();
    let before = capture(&memory, memory.read().unwrap()).unwrap();
    let mut state = State::default();
    let mut shared = None;
    execute(&mut state, &memory, &mut shared, 97).unwrap();
    assert_eq!(
        memory.0.lock().unwrap().writes,
        [
            "lock",
            "gpu0:core",
            "gpu0:memory",
            "gpu1:core",
            "gpu1:memory"
        ]
    );
    let changed = capture(&memory, memory.read().unwrap()).unwrap();
    assert_eq!(changed.locks.entries[2], before.locks.entries[2]);
    assert_eq!(
        raw(&changed.offsets[0]),
        RawOffsets {
            core_khz: Some(100_000),
            mem_khz: Some(-1_000)
        }
    );
    assert_eq!(
        shared,
        Some(GpuCheckpoint::new(
            before.devices[0],
            raw(&before.offsets[0])
        ))
    );
    let preview = state.preview(&memory, &memory);
    state
        .restore(&memory, preview.restore_revision.as_deref().unwrap())
        .unwrap();
    let after = capture(&memory, memory.read().unwrap()).unwrap();
    assert_eq!(after.locks, before.locks);
    assert_eq!(after.offsets, before.offsets);
    assert!(state.initial.is_none());
}

#[test]
fn all_devices_are_preflighted_before_the_first_write() {
    let memory = fixture();
    memory.0.lock().unwrap().offsets[1].core.max_khz = 0;
    let mut state = State::default();
    assert!(execute(&mut state, &memory, &mut None, 97).is_err());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(state.initial.is_none());
}

#[test]
fn preview_shows_mode_specific_driver_limits_without_writing() {
    let memory = fixture();
    memory.0.lock().unwrap().offsets[1].core.max_khz = 0;
    let preview = State::default().preview(&memory, &memory);
    let current = preview.current.unwrap();
    let ac = current
        .on_targets
        .iter()
        .find(|target| target.mode == 2 && target.on_ac)
        .unwrap();
    let dc = current
        .on_targets
        .iter()
        .find(|target| target.mode == 2 && !target.on_ac)
        .unwrap();
    assert!(
        ac.unavailable_reason
            .as_deref()
            .unwrap()
            .contains("超出允许范围")
    );
    assert!(dc.unavailable_reason.is_none());
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn partial_failure_restores_attempted_fields_in_reverse_order() {
    let memory = fixture();
    let before = capture(&memory, memory.read().unwrap()).unwrap();
    memory.0.lock().unwrap().fail_on = vec![3];
    let mut state = State::default();
    let mut shared = None;
    let error = execute(&mut state, &memory, &mut shared, 97)
        .unwrap_err()
        .to_string();
    assert!(error.contains("已恢复"));
    assert_eq!(
        memory.0.lock().unwrap().writes,
        [
            "lock",
            "gpu0:core",
            "gpu0:memory",
            "gpu0:memory",
            "gpu0:core",
            "lock"
        ]
    );
    let after = capture(&memory, memory.read().unwrap()).unwrap();
    assert_eq!(after.locks, before.locks);
    assert_eq!(after.offsets, before.offsets);
    assert!(state.initial.is_none());
    assert_eq!(shared, None);
}

#[test]
fn recovery_failure_keeps_baseline_and_retry_restores_it() {
    let memory = fixture();
    let before = capture(&memory, memory.read().unwrap()).unwrap();
    memory.0.lock().unwrap().fail_on = vec![3, 4];
    let mut state = State::default();
    let mut shared = None;
    assert!(
        format!(
            "{:#}",
            execute(&mut state, &memory, &mut shared, 97).unwrap_err()
        )
        .contains("恢复失败")
    );
    assert!(state.initial.is_some());
    let preview = state.preview(&memory, &memory);
    state
        .restore(&memory, preview.restore_revision.as_deref().unwrap())
        .unwrap();
    let after = capture(&memory, memory.read().unwrap()).unwrap();
    assert_eq!(after.locks, before.locks);
    assert_eq!(after.offsets, before.offsets);
}

#[test]
fn first_card_uses_the_existing_shared_gpu_baseline() {
    let memory = fixture();
    let mut state = State::default();
    let mut shared = Some(GpuCheckpoint {
        device: device(1),
        core_khz: Some(-20_000),
        mem_khz: Some(5_000),
    });
    execute(&mut state, &memory, &mut shared, 97).unwrap();
    assert_eq!(
        state.initial.as_ref().unwrap().offsets[0],
        shared.unwrap().raw()
    );
    let preview = state.preview(&memory, &memory);
    state
        .restore(&memory, preview.restore_revision.as_deref().unwrap())
        .unwrap();
    let current = memory.0.lock().unwrap();
    assert_eq!(raw(&current.offsets[0]), shared.unwrap().raw());
}

#[test]
fn changed_mode_mid_transaction_restores_without_sending_later_targets() {
    let memory = fixture();
    let expected = identity(&memory);
    let before = capture(&memory, memory.read().unwrap()).unwrap();
    let start = memory.0.lock().unwrap().reads;
    memory.0.lock().unwrap().change_mode_on_read = Some(start + 3);
    let mut state = State::default();
    assert!(
        state
            .execute(&memory, &memory, &mut None, &expected, 97, || Ok(()))
            .is_err()
    );
    assert_eq!(memory.0.lock().unwrap().writes, ["lock", "lock"]);
    assert_eq!(memory.0.lock().unwrap().locks, before.locks);
}

#[test]
fn canceled_permission_after_lock_rolls_back_before_returning() {
    let memory = fixture();
    let expected = identity(&memory);
    let mut calls = 0;
    let mut state = State::default();
    let result = state.execute(&memory, &memory, &mut None, &expected, 97, || {
        calls += 1;
        if calls == 4 {
            bail!("授权已撤销");
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(memory.0.lock().unwrap().writes, ["lock", "lock"]);
    assert!(state.initial.is_none());
}

#[test]
fn lock_preparation_read_failure_never_sends_target_or_recovery() {
    let memory = fixture();
    let expected = identity(&memory);
    let count = memory.0.lock().unwrap().lock_reads;
    memory.0.lock().unwrap().fail_lock_read_at = Some(count + 3);
    let mut state = State::default();
    let mut shared = None;
    assert!(
        state
            .execute(&memory, &memory, &mut shared, &expected, 97, || Ok(()))
            .is_err()
    );
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(state.initial.is_none());
    assert!(shared.is_none());
}

#[test]
fn stale_restore_and_changed_device_never_write() {
    let memory = fixture();
    let mut state = State::default();
    execute(&mut state, &memory, &mut None, 96).unwrap();
    let preview = state.preview(&memory, &memory);
    memory.0.lock().unwrap().locks.entries[0].value += 1;
    let count = memory.0.lock().unwrap().writes.len();
    assert!(
        state
            .restore(&memory, preview.restore_revision.as_deref().unwrap())
            .is_err()
    );
    assert_eq!(memory.0.lock().unwrap().writes.len(), count);
    memory.0.lock().unwrap().devices.swap(0, 1);
    assert!(state.restore(&memory, "old").is_err());
    assert_eq!(memory.0.lock().unwrap().writes.len(), count);
}

#[test]
fn turbo_off_only_changes_first_gpu_locks_and_dc_performance_only_changes_offsets() {
    let memory = fixture();
    let mut state = State::default();
    execute(&mut state, &memory, &mut None, 96).unwrap();
    assert_eq!(memory.0.lock().unwrap().writes, ["lock"]);
    memory.0.lock().unwrap().writes.clear();
    memory.0.lock().unwrap().source.on_ac = Some(false);
    execute(&mut state, &memory, &mut None, 97).unwrap();
    assert!(
        memory
            .0
            .lock()
            .unwrap()
            .writes
            .iter()
            .all(|name| name.starts_with("gpu"))
    );
}
