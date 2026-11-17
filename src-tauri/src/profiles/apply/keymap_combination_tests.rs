use super::super::perkey_tests::fixture as led_fixture;
use super::*;
use crate::lighting::perkey;
use std::sync::{Arc, Mutex, atomic::AtomicUsize};

fn attach_perkey(f: &mut Fixture) -> (perkey::Target, Arc<perkey::TestRuntime>) {
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[2][4] = 3;
        state.pages[2][32..36].copy_from_slice(&[12, 1, 2, 4]);
        state.pages[2][128..].fill(17);
        state.pages[3].fill(17);
    }
    let device = perkey::DeviceInfo {
        new_effects: true,
        language: 0,
        narrow_border: true,
        three_column: false,
        compact: Some(false),
        physical_three_column: Some(false),
    };
    let runtime = Arc::new(perkey::TestRuntime::default());
    let io = Arc::new(led_fixture::Memory {
        device: f.device.clone(),
        info: Mutex::new(device.clone()),
        identity: Mutex::new("isolated-combined-keyboard".into()),
        traffic: Mutex::new(led_fixture::Traffic::default()),
        watched_runtime: Mutex::new(Some(runtime.clone())),
        active_during_write: AtomicUsize::new(0),
    });
    f.perkey = perkey::Controller::default()
        .with_test_io(io)
        .with_test_runtime(runtime.clone());
    let target = serde_json::from_value(serde_json::json!({"keyboardType":3,"device":device,"values":{
        "enabled":true,"brightness":3,"speed":1,"pattern":{"kind":"ripple","reactive":true,"color":{"r":10,"g":20,"b":30}}}})).unwrap();
    (target, runtime)
}

#[test]
fn final_mapping_commit_failure_restores_perkey_and_exits_both_candidates_outside_hal() {
    let mut h = Harness::new();
    let (target, leds) = attach_perkey(&mut h.f);
    let original_pages = {
        let state = h.f.device.0.lock().unwrap();
        [state.pages[2], state.pages[3]]
    };
    let mut wanted = h.wanted(true);
    wanted.perkey_lighting = Some(target);
    h.io.lock_commit.store(true, Ordering::SeqCst);
    let steps = h.apply(&wanted).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "逐键按键响应" && step.ok)
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "提交映射与程序监听" && !step.ok)
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复逐键灯效" && step.ok)
    );
    assert_eq!(h.programs.active(), 0);
    assert_eq!(leds.active(), 0);
    assert_eq!(h.programs.exited.load(Ordering::SeqCst), 1);
    assert_eq!(h.programs.exited_without_gate.load(Ordering::SeqCst), 1);
    assert_eq!(leds.exited.load(Ordering::SeqCst), 1);
    assert_eq!(leds.exit_with_gate.load(Ordering::SeqCst), 1);
    let state = h.f.device.0.lock().unwrap();
    assert_eq!([state.pages[2], state.pages[3]], original_pages);
}
