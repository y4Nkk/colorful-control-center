use super::*;
#[path = "../../../profiles/apply/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use fixtures::Fixture;

fn setup(ac_mode: u8) -> Fixture {
    let f = Fixture::new();
    {
        let mut data = f.device.0.lock().unwrap();
        data.pages[1][1] = 2;
        data.pages[1][16] = ac_mode;
        data.bios[0] = 1;
        data.bios[16] |= 16;
    }
    f
}
fn options(f: &Fixture) {
    let current = boost::read(&f.hal).unwrap();
    boost::apply(
        &f.hal,
        &boost::Request {
            expected_revision: current.revision,
            values: boost::Values {
                turbo_fan: Some(true),
                dtt: None,
            },
        },
    )
    .unwrap();
}
#[test]
fn explicit_selection_saves_preference_before_command_and_restores_the_exact_original_byte() {
    let f = setup(241);
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 3).unwrap();
    assert_eq!(plan.target.ac_mode, Some(3));
    plan.execute(&f.hal).unwrap();
    {
        let data = f.device.0.lock().unwrap();
        assert_eq!(data.pages[1][16], 3);
        let save = data
            .events
            .iter()
            .position(|event| event.starts_with("PAGE1:16:"))
            .unwrap();
        let command = data
            .events
            .iter()
            .position(|event| event == "MODE:3")
            .unwrap();
        assert!(save < command);
    }
    let target = restore_target(&f.hal).unwrap().unwrap();
    assert_eq!(target.ac_mode, Some(241));
    restore(&f.hal, &target.revision).unwrap();
    let data = f.device.0.lock().unwrap();
    assert_eq!(data.pages[1][16], 241);
    assert_eq!(data.pages[1][1], 2);
}
#[test]
fn preference_changes_after_checkpoint_reject_before_any_output() {
    let f = setup(1);
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 3).unwrap();
    f.device.0.lock().unwrap().pages[1][16] = 0;
    let events = f.device.0.lock().unwrap().events.clone();
    assert!(plan.execute(&f.hal).is_err());
    assert_eq!(f.device.0.lock().unwrap().events, events);
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}
#[test]
fn cancellation_before_output_does_not_expand_an_existing_options_restore_target() {
    let f = setup(1);
    let _gate = f.hal.begin_write().unwrap();
    options(&f);
    let original = restore_target(&f.hal).unwrap().unwrap();
    let events = f.device.0.lock().unwrap().events.clone();
    let plan = checkpoint(&f.hal, 3).unwrap();
    let mut checks = 0;
    assert!(
        plan.execute_guarded(&f.hal, &mut || {
            checks += 1;
            if checks == 4 {
                bail!("隔离首个模式输出前撤销");
            }
            Ok(())
        })
        .is_err()
    );
    let after = restore_target(&f.hal).unwrap().unwrap();
    assert_eq!(after.revision, original.revision);
    assert_eq!(after.ac_mode, None);
    assert_eq!(f.device.0.lock().unwrap().events, events);
}
#[test]
fn options_do_not_own_or_restore_the_ac_preference() {
    let f = setup(1);
    let _gate = f.hal.begin_write().unwrap();
    options(&f);
    let target = restore_target(&f.hal).unwrap().unwrap();
    assert_eq!(target.ac_mode, None);
    assert_eq!(f.device.0.lock().unwrap().pages[1][16], 1);
    f.device.0.lock().unwrap().pages[1][16] = 3;
    restore(&f.hal, &target.revision).unwrap();
    assert_eq!(f.device.0.lock().unwrap().pages[1][16], 3);
}
#[test]
fn selecting_after_options_adds_only_the_first_ac_baseline_and_invalidates_old_restore() {
    let f = setup(1);
    let _gate = f.hal.begin_write().unwrap();
    options(&f);
    let old = restore_target(&f.hal).unwrap().unwrap();
    f.device.0.lock().unwrap().pages[1][16] = 3;
    checkpoint(&f.hal, 0).unwrap().execute(&f.hal).unwrap();
    let target = restore_target(&f.hal).unwrap().unwrap();
    assert_eq!(target.ac_mode, Some(3));
    assert_eq!(target.mode, 2);
    assert!(!target.turbo);
    let events = f.device.0.lock().unwrap().events.clone();
    assert!(restore(&f.hal, &old.revision).is_err());
    assert_eq!(f.device.0.lock().unwrap().events, events);
    checkpoint(&f.hal, 1).unwrap().execute(&f.hal).unwrap();
    assert_eq!(
        restore_target(&f.hal).unwrap().unwrap().revision,
        target.revision
    );
    restore(&f.hal, &target.revision).unwrap();
    let data = f.device.0.lock().unwrap();
    assert_eq!(data.pages[1][16], 3);
    assert_eq!(data.pages[1][1], 2);
    assert_eq!(data.pages[4][8], 0);
}
#[test]
fn automatic_mode_targets_and_restore_leave_the_ac_preference_untouched() {
    let f = setup(2);
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint_for_supply(&f.hal, 1, false).unwrap();
    assert_eq!(plan.target.ac_mode, None);
    plan.execute(&f.hal).unwrap();
    assert_eq!(f.device.0.lock().unwrap().pages[1][16], 2);
    f.device.0.lock().unwrap().pages[1][16] = 3;
    let target = restore_target(&f.hal).unwrap().unwrap();
    assert_eq!(target.ac_mode, None);
    restore(&f.hal, &target.revision).unwrap();
    assert_eq!(f.device.0.lock().unwrap().pages[1][16], 3);
}
