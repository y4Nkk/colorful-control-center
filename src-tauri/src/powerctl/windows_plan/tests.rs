use super::*;
use crate::{
    battery::calibration::RECORD_NAME,
    hal::{
        oem_dchu::Dchu,
        power_plan::{BatteryAction, BatteryLevel, PowerAccess},
    },
};
use anyhow::{Context, bail};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

const BALANCED: u128 = 0x381b4222_f694_41f0_9685_ff5bb260df2e;
const HIGH: u128 = 0x22222222_2222_2222_2222_222222222222;
const CUSTOM: u128 = 0x55555555_5555_5555_5555_555555555555;

struct Memory(Mutex<State>);
struct State {
    plans: BTreeMap<u128, String>,
    active: u128,
    activations: Vec<u128>,
    write_allowed: bool,
    fail_readback: bool,
}

impl PowerAccess for Memory {
    fn schemes(&self) -> Result<Vec<u128>> {
        Ok(self.0.lock().unwrap().plans.keys().copied().collect())
    }
    fn scheme_name(&self, scheme: u128) -> Result<String> {
        self.0
            .lock()
            .unwrap()
            .plans
            .get(&scheme)
            .cloned()
            .context("隔离计划不存在")
    }
    fn active(&self) -> Result<u128> {
        Ok(self.0.lock().unwrap().active)
    }
    fn check_active_scheme_write(&self) -> Result<()> {
        ensure!(
            self.0.lock().unwrap().write_allowed,
            "Windows 组策略禁止切换 (1260)"
        );
        Ok(())
    }
    fn activate(&self, scheme: u128) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        state.activations.push(scheme);
        state.active = scheme;
        if std::mem::take(&mut state.fail_readback) {
            bail!("隔离回读失败");
        }
        Ok(())
    }
    fn lid_action(&self, _: u128) -> Result<[u32; 2]> {
        unreachable!()
    }
    fn battery_action(&self, _: u128, _: BatteryAction) -> Result<u32> {
        unreachable!()
    }
    fn battery_level(&self, _: u128, _: BatteryLevel) -> Result<[u32; 2]> {
        unreachable!()
    }
    fn set_battery_level(&self, _: u128, _: BatteryLevel, _: bool, _: u32) -> Result<()> {
        unreachable!()
    }
    fn usb(&self, _: u128) -> Result<[u32; 2]> {
        unreachable!()
    }
    fn set_usb(&self, _: u128, _: bool, _: u32) -> Result<()> {
        unreachable!()
    }
    fn aspm(&self, _: u128) -> Result<[u32; 2]> {
        unreachable!()
    }
    fn set_aspm(&self, _: u128, _: bool, _: u32) -> Result<()> {
        unreachable!()
    }
    fn brightness(&self, _: u128) -> Result<[u32; 2]> {
        unreachable!()
    }
    fn set_brightness(&self, _: u128, _: bool, _: u32) -> Result<()> {
        unreachable!()
    }
}

fn fixture() -> (Hal, Arc<Memory>) {
    unsafe extern "system" fn packet(_: i32, _: *mut u8) -> i32 {
        unreachable!()
    }
    unsafe extern "system" fn read(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
        unreachable!()
    }
    unsafe extern "system" fn send(_: i32, _: *const u8, _: i32) -> i32 {
        unreachable!()
    }
    unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
        unreachable!()
    }
    let mut hal = Hal::test_instance(Dchu::test_io(packet, read, send, write), None);
    let io = Arc::new(Memory(Mutex::new(State {
        plans: [
            (BALANCED, "平衡".into()),
            (HIGH, "高性能".into()),
            (CUSTOM, "自定义办公 🪟".into()),
        ]
        .into(),
        active: HIGH,
        activations: Vec::new(),
        write_allowed: true,
        fail_readback: false,
    })));
    hal.system_power = io.clone();
    (hal, io)
}

fn request(profile: &Profile, target: u128) -> Request {
    Request {
        expected_revision: profile.revision.clone(),
        target_id: SchemeId(target),
    }
}

#[test]
fn switches_installed_plans_using_the_latest_readback() {
    let (hal, io) = fixture();
    let profile = read(&hal).unwrap();
    assert_eq!(profile.active_id, SchemeId(HIGH));
    let first_revision = profile.revision.clone();
    assert!(
        profile
            .plans
            .iter()
            .any(|plan| plan.id == SchemeId(CUSTOM) && plan.name == "自定义办公 🪟")
    );
    apply(&hal, &request(&profile, HIGH)).unwrap();
    apply(&hal, &request(&profile, CUSTOM)).unwrap();
    let profile = read(&hal).unwrap();
    assert_eq!(profile.active_id, SchemeId(CUSTOM));
    apply(&hal, &request(&profile, BALANCED)).unwrap();
    let profile = read(&hal).unwrap();
    assert_eq!(profile.active_id, SchemeId(BALANCED));
    apply(&hal, &request(&profile, HIGH)).unwrap();
    let profile = read(&hal).unwrap();
    assert_eq!(profile.active_id, SchemeId(HIGH));
    assert_eq!(profile.revision, first_revision);
    assert_eq!(io.0.lock().unwrap().activations, [CUSTOM, BALANCED, HIGH]);
}

#[test]
fn rejects_stale_active_plan_stale_catalog_and_unknown_targets() {
    let (hal, io) = fixture();
    let profile = read(&hal).unwrap();
    io.0.lock().unwrap().active = BALANCED;
    assert!(apply(&hal, &request(&profile, CUSTOM)).is_err());

    let profile = read(&hal).unwrap();
    io.0.lock().unwrap().plans.remove(&CUSTOM);
    assert!(apply(&hal, &request(&profile, CUSTOM)).is_err());
    let profile = read(&hal).unwrap();
    assert!(apply(&hal, &request(&profile, CUSTOM)).is_err());
    assert!(io.0.lock().unwrap().activations.is_empty());
}

#[test]
fn an_activation_readback_error_is_returned_and_the_next_read_reflects_the_system() {
    let (hal, io) = fixture();
    let profile = read(&hal).unwrap();
    io.0.lock().unwrap().fail_readback = true;
    let error = apply(&hal, &request(&profile, CUSTOM)).unwrap_err();
    assert_eq!(error.to_string(), "隔离回读失败");
    let profile = read(&hal).unwrap();
    assert_eq!(profile.active_id, SchemeId(CUSTOM));
    apply(&hal, &request(&profile, HIGH)).unwrap();
    assert_eq!(io.0.lock().unwrap().activations, [CUSTOM, HIGH]);
}

#[test]
fn calibration_record_error_allows_reads_but_rejects_activation() {
    let (mut hal, io) = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(RECORD_NAME);
    std::fs::write(&path, b"{broken").unwrap();
    hal.bind_calibration_record(path);
    let profile = read(&hal).unwrap();
    assert!(profile.blocked_reason.is_some());
    assert!(apply(&hal, &request(&profile, BALANCED)).is_err());
    assert!(io.0.lock().unwrap().activations.is_empty());
}

#[test]
fn policy_changes_invalidate_the_preview_and_block_activation() {
    let (hal, io) = fixture();
    let preview = read(&hal).unwrap();
    io.0.lock().unwrap().write_allowed = false;
    let blocked = read(&hal).unwrap();
    assert_eq!(blocked.active_id, preview.active_id);
    assert_ne!(blocked.revision, preview.revision);
    assert!(blocked.blocked_reason.as_deref().unwrap().contains("1260"));
    let failure = apply(&hal, &request(&preview, CUSTOM)).unwrap_err();
    assert!(failure.to_string().contains("1260"));
    assert!(io.0.lock().unwrap().activations.is_empty());
}

#[test]
fn ipc_only_accepts_canonical_plan_guids() {
    let id = "381b4222-f694-41f0-9685-ff5bb260df2e";
    let body = |target| serde_json::json!({ "expectedRevision": "preview", "targetId": target });
    let request = serde_json::from_value::<Request>(body(id)).unwrap();
    assert_eq!(request.target_id, SchemeId(BALANCED));
    assert_eq!(serde_json::to_value(request.target_id).unwrap(), id);
    for invalid in [
        "381b4222f69441f09685ff5bb260df2e",
        "{381b4222-f694-41f0-9685-ff5bb260df2e}",
        "381B4222-F694-41F0-9685-FF5BB260DF2E",
        "00000000-0000-0000-0000-000000000000",
    ] {
        assert!(serde_json::from_value::<Request>(body(invalid)).is_err());
    }
}
