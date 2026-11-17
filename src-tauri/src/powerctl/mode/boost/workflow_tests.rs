use super::*;
#[path = "../../../profiles/apply/fixtures.rs"]
#[allow(dead_code)] // 共用档案夹具，本组不使用GPU控制器。
mod fixtures;
use fixtures::Fixture;
fn setup() -> Fixture {
    let mut f = Fixture::new();
    {
        let mut s = f.device.0.lock().unwrap();
        s.pages[1][1] = 2;
        s.bios[0] = 1;
        s.bios[16] |= 16;
        s.bios[11] |= 16;
    }
    f.hal.set_test_dtt_driver(std::sync::Arc::new(|| Ok(true)));
    f
}
fn request(f: &Fixture, turbo: bool, dtt: bool) -> Request {
    let revision = f
        .hal
        .with_dchu(|io| preview(io, Some(true)).map(|p| p.revision))
        .unwrap();
    Request {
        expected_revision: revision,
        values: Values {
            turbo_fan: Some(turbo),
            dtt: Some(dtt),
        },
    }
}
#[test]
fn real_workflow_sends_independent_dtt_without_touching_windows_or_fan() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    apply(&f.hal, &request(&f, false, true)).unwrap();
    {
        let s = f.device.0.lock().unwrap();
        assert_eq!(s.pages[1][32], 1);
        assert_eq!(s.pages[4][8], 0);
        assert!(s.events.iter().any(|e| e == "MODE:130"));
        assert!(
            !s.events.iter().any(|e| e.starts_with("PAGE4:5:")
                || e.starts_with("ASPM")
                || e.starts_with("PLAN"))
        );
    }
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
    super::super::restore(
        &f.hal,
        &super::super::workflow::restore_target(&f.hal)
            .unwrap()
            .unwrap()
            .revision,
    )
    .unwrap();
    assert_eq!(f.device.0.lock().unwrap().pages[1][32], 0);
}
#[test]
fn real_turbo_fan_failure_restores_saved_flags_and_keeps_initial_retry() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().fail_fan_mode = true;
    let error = apply(&f.hal, &request(&f, true, true))
        .unwrap_err()
        .to_string();
    assert!(error.contains("失败") || error.contains("SetDCHU"));
    {
        let s = f.device.0.lock().unwrap();
        assert_eq!(s.pages[4][8], 0);
        assert_eq!(s.pages[1][32], 0);
        assert_eq!(s.pages[4][5], 0);
        assert!(s.events.iter().any(|e| e == "FAN_MODE_FAILURE"));
    }
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
    super::super::restore(
        &f.hal,
        &super::super::workflow::restore_target(&f.hal)
            .unwrap()
            .unwrap()
            .revision,
    )
    .unwrap();
}
#[test]
fn command_and_compensation_failure_keep_shared_baseline_for_user_retry() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().fail_mode = true;
    let error = apply(&f.hal, &request(&f, true, true))
        .unwrap_err()
        .to_string();
    assert!(error.contains("恢复失败"));
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
    f.device.0.lock().unwrap().fail_mode = false;
    super::super::restore(
        &f.hal,
        &super::super::workflow::restore_target(&f.hal)
            .unwrap()
            .unwrap()
            .revision,
    )
    .unwrap();
    let s = f.device.0.lock().unwrap();
    assert_eq!(s.pages[4][8], 0);
    assert_eq!(s.pages[1][32], 0);
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}
#[test]
fn later_mode_change_restores_the_first_shared_options_baseline() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    apply(&f.hal, &request(&f, true, false)).unwrap();
    super::super::checkpoint(&f.hal, 3)
        .unwrap()
        .execute(&f.hal)
        .unwrap();
    super::super::restore(
        &f.hal,
        &super::super::workflow::restore_target(&f.hal)
            .unwrap()
            .unwrap()
            .revision,
    )
    .unwrap();
    let s = f.device.0.lock().unwrap();
    assert_eq!(s.pages[1][1], 2);
    assert_eq!(s.pages[4][8], 0);
    assert_eq!(s.pages[1][32], 0);
}

#[test]
fn revoked_before_first_mode_output_does_not_write_or_create_baseline() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = super::super::checkpoint(&f.hal, 3).unwrap();
    let mut checks = 0;
    let result = plan.execute_guarded(&f.hal, &mut || {
        checks += 1;
        if checks == 4 {
            bail!("隔离许可撤销");
        }
        Ok(())
    });
    assert!(result.is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}

#[test]
fn revoked_after_mode_command_compensates_the_original_state() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = super::super::checkpoint(&f.hal, 3).unwrap();
    let result = plan.execute_guarded(&f.hal, &mut || {
        if f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event == "MODE:3")
        {
            bail!("隔离许可撤销");
        }
        Ok(())
    });
    assert!(result.is_err());
    let data = f.device.0.lock().unwrap();
    assert!(data.events.iter().any(|event| event == "MODE:3"));
    assert!(data.events.iter().any(|event| event == "MODE:2"));
    assert_eq!(data.pages[1][1], 2);
}

#[test]
fn revoked_after_ac_preference_save_restores_it_before_sending_any_target_mode() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = super::super::checkpoint(&f.hal, 3).unwrap();
    let result = plan.execute_guarded(&f.hal, &mut || {
        if f.device.0.lock().unwrap().pages[1][16] == 3 {
            bail!("隔离偏好保存后撤销");
        }
        Ok(())
    });
    assert!(result.is_err());
    let data = f.device.0.lock().unwrap();
    assert_eq!(data.pages[1][16], 0);
    assert_eq!(data.pages[1][1], 2);
    assert!(!data.events.iter().any(|event| event == "MODE:3"));
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}
