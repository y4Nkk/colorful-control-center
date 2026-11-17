use super::*;
use crate::gpuctl::turbo::mode::{Policy, Prepared, RawState, Write};
#[path = "driver_tests.rs"]
mod driver_tests;
#[path = "linked_tests.rs"]
mod linked_tests;
#[path = "session_tests.rs"]
mod session_tests;

struct First(std::sync::Arc<Memory>);
impl crate::hal::nvapi::transaction::PstatesIo for First {
    fn read(&self) -> Result<ParsedPstates> {
        self.0.offsets(&device(1))
    }
    fn write(&self, core: Option<i32>, memory: Option<i32>) -> Result<()> {
        self.0.write_offsets(&device(1), core, memory)
    }
}
fn isolated_controller(memory: std::sync::Arc<Memory>) -> crate::gpuctl::GpuController {
    crate::gpuctl::GpuController::test_with_sources(
        std::sync::Arc::new(First(memory.clone())),
        memory,
    )
}

struct NoBase<'a>(&'a Memory);
impl TurboIo for NoBase<'_> {
    fn devices(&self) -> Result<Vec<DeviceId>> {
        self.0.devices()
    }
    fn primary(&self) -> Result<DeviceId> {
        self.0.primary()
    }
    fn base_clock(&self, _: &DeviceId) -> Result<u32> {
        bail!("模式调校不得查询温控基础频率")
    }
    fn locks(&self, id: &DeviceId) -> Result<LockTable> {
        self.0.locks(id)
    }
    fn write_locks(&self, id: &DeviceId, value: &LockTable) -> Result<()> {
        self.0.write_locks(id, value)
    }
    fn offsets(&self, id: &DeviceId) -> Result<ParsedPstates> {
        self.0.offsets(id)
    }
    fn write_offsets(&self, id: &DeviceId, core: Option<i32>, memory: Option<i32>) -> Result<()> {
        self.0.write_offsets(id, core, memory)
    }
}

fn policy(memory: &Memory) -> Policy {
    let data = memory.0.lock().unwrap();
    Policy {
        capabilities: Some(data.source.capabilities),
        limits: data.source.limits,
        saved: data.source.saved,
        gpu_oc: Some(true),
        simple_gpu_oc: false,
        ready: true,
        gpu_count: data.devices.len() as u8,
        cached_device_id: data.devices[0].extended as u16,
    }
}
fn prepare(memory: &Memory) -> (Policy, Prepared) {
    let policy = policy(memory);
    let prepared = Prepared::prepare(&NoBase(memory), memory, &policy, 3, true, false)
        .unwrap()
        .unwrap();
    (policy, prepared)
}
fn switch_mode(memory: &Memory) {
    memory.0.lock().unwrap().source.mode = 3;
}

#[test]
fn mode_prepares_without_base_clock_persists_before_output_and_restores_raw_fields() {
    let memory = fixture();
    memory.0.lock().unwrap().source.limits.thermal_mhz = 0;
    let (policy, prepared) = prepare(&memory);
    assert!(memory.0.lock().unwrap().writes.is_empty());
    switch_mode(&memory);
    let mut state = State::default();
    let mut shared = None;
    let mut persisted = None;
    prepared
        .execute(
            &NoBase(&memory),
            &memory,
            &mut state,
            &mut shared,
            &policy,
            &mut |write| {
                assert!(memory.0.lock().unwrap().writes.is_empty());
                let restored: Write = serde_json::from_slice(&serde_json::to_vec(write)?)?;
                restored.validate()?;
                persisted = Some(restored);
                Ok(())
            },
            &mut || Ok(()),
        )
        .unwrap();
    assert_eq!(persisted.unwrap(), prepared.write);
    assert_eq!(RawState::read(&memory).unwrap(), prepared.write.after);
    let unowned = memory.0.lock().unwrap().locks.entries[2];
    let after = RawState::read(&memory).unwrap();
    prepared
        .write
        .before
        .restore(&NoBase(&memory), &after, &mut || Ok(()))
        .unwrap();
    assert_eq!(RawState::read(&memory).unwrap(), prepared.write.before);
    assert_eq!(memory.0.lock().unwrap().locks.entries[2], unowned);
    assert_eq!(memory.0.lock().unwrap().writes.last().unwrap(), "lock");
}

#[test]
fn failed_persistence_prevents_all_gpu_output_and_shared_baselines() {
    let memory = fixture();
    let (policy, prepared) = prepare(&memory);
    switch_mode(&memory);
    let mut state = State::default();
    let mut shared = None;
    let error = prepared
        .execute(
            &memory,
            &memory,
            &mut state,
            &mut shared,
            &policy,
            &mut |_| bail!("隔离恢复记录落盘失败"),
            &mut || Ok(()),
        )
        .unwrap_err();
    assert!(error.to_string().contains("落盘失败"));
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(state.initial.is_none() && shared.is_none());
}

#[test]
fn actual_mode_source_or_raw_state_changes_reject_before_persistence() {
    let memory = fixture();
    let (policy, prepared) = prepare(&memory);
    let mut state = State::default();
    let mut shared = None;
    assert!(
        prepared
            .execute(
                &memory,
                &memory,
                &mut state,
                &mut shared,
                &policy,
                &mut |_| panic!("旧模式不能保存执行记录"),
                &mut || Ok(())
            )
            .is_err()
    );
    switch_mode(&memory);
    memory.0.lock().unwrap().source.on_ac = Some(false);
    assert!(
        prepared
            .execute(
                &memory,
                &memory,
                &mut state,
                &mut shared,
                &policy,
                &mut |_| panic!("旧供电不能保存执行记录"),
                &mut || Ok(())
            )
            .is_err()
    );
    memory.0.lock().unwrap().source.on_ac = Some(true);
    memory.0.lock().unwrap().offsets[0].core.delta_khz += 1;
    assert!(
        prepared
            .execute(
                &memory,
                &memory,
                &mut state,
                &mut shared,
                &policy,
                &mut |_| panic!("旧偏移不能保存执行记录"),
                &mut || Ok(())
            )
            .is_err()
    );
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn revocation_after_the_journal_is_saved_still_prevents_output() {
    let memory = fixture();
    let (policy, prepared) = prepare(&memory);
    switch_mode(&memory);
    let mut saved = false;
    let mut permits = 0;
    let mut state = State::default();
    let mut shared = None;
    assert!(
        prepared
            .execute(
                &memory,
                &memory,
                &mut state,
                &mut shared,
                &policy,
                &mut |_| {
                    saved = true;
                    Ok(())
                },
                &mut || {
                    permits += 1;
                    if permits == 2 {
                        bail!("隔离许可撤销");
                    }
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(saved && memory.0.lock().unwrap().writes.is_empty());
    assert!(state.initial.is_none() && shared.is_none());
}

#[test]
fn partial_gpu_failure_reuses_existing_compensation_and_exact_khz_baselines() {
    let memory = fixture();
    let (policy, prepared) = prepare(&memory);
    switch_mode(&memory);
    memory.0.lock().unwrap().fail_on = vec![2];
    let mut state = State::default();
    let mut shared = None;
    let mut saved = false;
    assert!(
        prepared
            .execute(
                &memory,
                &memory,
                &mut state,
                &mut shared,
                &policy,
                &mut |_| {
                    saved = true;
                    Ok(())
                },
                &mut || Ok(())
            )
            .is_err()
    );
    assert!(saved);
    assert_eq!(RawState::read(&memory).unwrap(), prepared.write.before);
    assert!(state.initial.is_none() && shared.is_none());
}

#[test]
fn the_first_gpu_shared_baseline_is_reused_in_mode_tuning() {
    let memory = fixture();
    let (policy, prepared) = prepare(&memory);
    switch_mode(&memory);
    let mut state = State::default();
    let original = GpuCheckpoint::new(
        device(1),
        RawOffsets {
            core_khz: Some(-12_345),
            mem_khz: None,
        },
    );
    let mut shared = Some(original);
    prepared
        .execute(
            &memory,
            &memory,
            &mut state,
            &mut shared,
            &policy,
            &mut |_| Ok(()),
            &mut || Ok(()),
        )
        .unwrap();
    assert_eq!(shared.unwrap().core_khz, original.core_khz);
    assert_eq!(
        state.initial.as_ref().unwrap().offsets[0].core_khz,
        original.core_khz
    );
}

#[test]
fn source_cache_matches_extended_device_id_and_does_not_guess_pci_device_bits() {
    let memory = fixture();
    let mut policy = policy(&memory);
    policy.cached_device_id = device(1).pci_device as u16;
    assert!(Prepared::prepare(&memory, &memory, &policy, 3, true, false).is_err());
    policy.cached_device_id = 0;
    assert!(
        Prepared::prepare(&memory, &memory, &policy, 3, true, false)
            .unwrap()
            .is_some()
    );
    policy.gpu_count = 3;
    assert!(Prepared::prepare(&memory, &memory, &policy, 3, true, false).is_err());
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn skipped_modes_do_not_query_gpu_or_source_state() {
    let memory = fixture();
    let mut policy = policy(&memory);
    policy.ready = false;
    assert!(
        Prepared::prepare(&memory, &memory, &policy, 3, true, false)
            .unwrap()
            .is_none()
    );
    policy.ready = true;
    assert!(
        Prepared::prepare(&memory, &memory, &policy, 3, true, true)
            .unwrap()
            .is_none()
    );
    assert_eq!(memory.0.lock().unwrap().reads, 0);
    assert_eq!(memory.0.lock().unwrap().lock_reads, 0);
}

#[test]
fn raw_restore_requires_a_current_preview_and_does_not_write_matching_fields() {
    let memory = fixture();
    let (_, prepared) = prepare(&memory);
    let before = prepared.write.before;
    before.restore(&memory, &before, &mut || Ok(())).unwrap();
    assert!(memory.0.lock().unwrap().writes.is_empty());
    memory.0.lock().unwrap().offsets[0].core.delta_khz += 1;
    assert!(before.restore(&memory, &before, &mut || Ok(())).is_err());
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn durable_write_sets_reject_unknown_fields_and_identify_outside_mutations() {
    let memory = fixture();
    let (_, prepared) = prepare(&memory);
    let write = prepared.write;
    assert!(write.may_recover(&write.before) && write.may_recover(&write.after));
    let mut partial = write.before.clone();
    partial.offsets[0] = write.after.offsets[0];
    assert!(write.may_recover(&partial));
    partial.offsets[0].core_khz = Some(123_456);
    assert!(!write.may_recover(&partial));
    let mut json = serde_json::to_value(&write).unwrap();
    json["before"]["devices"][0]["handle"] = serde_json::json!(123);
    assert!(serde_json::from_value::<Write>(json).is_err());
    let mut invalid = write;
    invalid.after.devices.reverse();
    assert!(invalid.validate().is_err());
}
