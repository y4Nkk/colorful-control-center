use super::*;

fn dim(io: &Memory, fixture: &Fixture) -> String {
    let identity = fixture.identity(io);
    event(io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    identity
}
#[test]
fn natural_end_keeps_percent_and_first_values_until_explicit_restore() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    let stale = preview(&io, &fixture.0).restore_revision.unwrap();
    event(&io, &fixture.0, &identity, 60, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 75]);
    assert_eq!(io.0.borrow().cache, [0, 20]);
    assert_eq!(io.0.borrow().signals, [2, 3]);
    assert!(restore(&io, &fixture.0, &stale).is_err());
    let record = fixture.0.load().unwrap().unwrap();
    assert_eq!(record.baseline.levels, [75, 45]);
    assert_eq!(record.baseline.cache, [0, 23]);
    restore(
        &io,
        &fixture.0,
        &preview(&io, &fixture.0).restore_revision.unwrap(),
    )
    .unwrap();
    assert_eq!(io.0.borrow().levels, [75, 45]);
    assert_eq!(io.0.borrow().cache, [0, 23]);
}
#[test]
fn natural_end_uses_current_nonzero_ac_instead_of_saved_brightness() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    io.0.borrow_mut().levels = [52, 37];
    assert!(preview(&io, &fixture.0).restore_revision.is_none());
    event(&io, &fixture.0, &identity, 60, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().levels, [52, 52]);
    assert_eq!(io.0.borrow().cache, [0, 20]);
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 45]);
}
#[test]
fn repeated_setting_caches_current_ac_without_dimming_it_again() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    io.0.borrow_mut().levels = [52, 37];
    io.0.borrow_mut().power.percent = Some(18);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().levels, [52, 37]);
    assert_eq!(io.0.borrow().cache, [52, 18]);
    assert_eq!(fixture.0.load().unwrap().unwrap().baseline.levels, [75, 45]);
}
#[test]
fn zero_saved_ac_does_not_trigger_natural_restore_or_clear_saved_percent() {
    let io = Memory::new();
    let fixture = Fixture::new();
    io.0.borrow_mut().levels = [0, 45];
    let identity = dim(&io, &fixture);
    assert_eq!(io.0.borrow().cache, [0, 20]);
    io.0.borrow_mut().power.ac_online = Some(true);
    let writes = io.0.borrow().writes;
    assert!(
        check(&io, &fixture.0, &identity, &mut || Ok(()))
            .unwrap()
            .is_none()
    );
    event(&io, &fixture.0, &identity, 60, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().writes, writes);
    assert_eq!(io.0.borrow().levels, [0, 0]);
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [0, 45]);
}
#[test]
fn repeated_cycles_keep_the_first_full_baseline() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    for percent in [19, 18] {
        event(&io, &fixture.0, &identity, 60, &mut || Ok(())).unwrap();
        io.0.borrow_mut().power.percent = Some(percent);
        event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
        assert_eq!(io.0.borrow().levels, [0, 0]);
    }
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 45]);
    assert_eq!(io.0.borrow().cache, [0, 23]);
}
#[test]
fn failed_natural_end_compensates_only_this_attempt_and_can_retry() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    let previous = fixture.0.load().unwrap().unwrap();
    let next = io.0.borrow().writes + 1;
    io.0.borrow_mut().failures = vec![next];
    assert!(event(&io, &fixture.0, &identity, 60, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().levels, [0, 0]);
    assert_eq!(io.0.borrow().cache, [75, 20]);
    assert_eq!(fixture.0.load().unwrap().unwrap(), previous);
    io.0.borrow_mut().failures.clear();
    event(&io, &fixture.0, &identity, 60, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 75]);
}
#[test]
fn interrupted_end_keeps_pending_values_and_initial_restore_is_retryable() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    let next = io.0.borrow().writes + 1;
    io.0.borrow_mut().failures = vec![next, next + 1];
    assert!(event(&io, &fixture.0, &identity, 60, &mut || Ok(())).is_err());
    assert!(fixture.0.load().unwrap().unwrap().pending.is_some());
    io.0.borrow_mut().failures.clear();
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 45]);
    assert_eq!(io.0.borrow().cache, [0, 23]);
}
#[test]
fn permission_revoked_after_output_compensates_without_resuming_outputs() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    let writes = io.0.borrow().writes;
    assert!(
        event(&io, &fixture.0, &identity, 60, &mut || {
            if io.0.borrow().writes > writes {
                bail!("许可已撤销");
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(io.0.borrow().levels, [0, 0]);
    assert_eq!(io.0.borrow().cache, [75, 20]);
    assert!(fixture.0.load().unwrap().unwrap().pending.is_none());
}
#[test]
fn changed_firmware_capability_blocks_recovery_before_any_output() {
    let io = Memory::new();
    let fixture = Fixture::new();
    dim(&io, &fixture);
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    io.0.borrow_mut().rule.customer = 5;
    let writes = io.0.borrow().writes;
    assert!(restore(&io, &fixture.0, &revision).is_err());
    assert_eq!(io.0.borrow().writes, writes);
    assert!(preview(&io, &fixture.0).restore_revision.is_none());
}
#[test]
fn old_record_shape_is_rejected_without_hardware_output() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    let record = fixture.0.load().unwrap().unwrap();
    let old = serde_json::json!({"version":1,"phase":"applied","baseline":record.baseline,
        "levelsOwned":[true,true],"cacheOwned":[true,true],"cacheTarget":[75,20],"pendingPercent":null});
    std::fs::write(&fixture.0.0, serde_json::to_vec(&old).unwrap()).unwrap();
    let writes = io.0.borrow().writes;
    assert!(fixture.0.load().is_err());
    assert!(event(&io, &fixture.0, &identity, 60, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, writes);
}

#[test]
fn auto_end_rechecks_recovery_condition_between_fields() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    io.0.borrow_mut().power.ac_online = Some(true);
    let writes = io.0.borrow().writes;
    assert!(
        check(&io, &fixture.0, &identity, &mut || {
            if io.0.borrow().writes > writes {
                io.0.borrow_mut().power.ac_online = Some(false);
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(io.0.borrow().levels, [0, 0]);
    assert_eq!(io.0.borrow().cache, [75, 20]);
}
#[test]
fn final_read_catches_a_previous_field_reverted_to_its_before_value() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = fixture.identity(&io);
    let mut changed = false;
    assert!(
        event(&io, &fixture.0, &identity, 59, &mut || {
            if !changed && io.0.borrow().writes == 4 {
                changed = true;
                io.0.borrow_mut().levels[0] = 75;
            }
            Ok(())
        })
        .is_err()
    );
    assert!(changed);
    assert_eq!(io.0.borrow().levels, [75, 45]);
    assert_eq!(io.0.borrow().cache, [0, 23]);
    assert!(fixture.0.load().unwrap().is_none());
}
#[test]
fn old_record_version_is_rejected_without_defaults_or_outputs() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = dim(&io, &fixture);
    let mut json = serde_json::to_value(fixture.0.load().unwrap().unwrap()).unwrap();
    json["version"] = 1.into();
    std::fs::write(&fixture.0.0, serde_json::to_vec(&json).unwrap()).unwrap();
    let writes = io.0.borrow().writes;
    assert!(fixture.0.load().is_err());
    assert!(event(&io, &fixture.0, &identity, 60, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, writes);
}

#[test]
fn backend_preview_and_execution_share_original_setting_and_ending_targets() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let first = preview(&io, &fixture.0);
    assert_eq!(first.setting_target, Some([0, 0]));
    assert_eq!(first.ending_target, Some([75, 45]));
    io.0.borrow_mut().power.ac_online = Some(true);
    assert_eq!(preview(&io, &fixture.0).setting_target, Some([75, 45]));
    io.0.borrow_mut().power.ac_online = Some(false);
    let identity = dim(&io, &fixture);
    io.0.borrow_mut().levels = [52, 37];
    let repeated = preview(&io, &fixture.0);
    assert_eq!(repeated.setting_target, Some([52, 37]));
    assert_eq!(repeated.ending_target, Some([52, 52]));
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().levels, repeated.setting_target.unwrap());
    event(&io, &fixture.0, &identity, 60, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().levels, repeated.ending_target.unwrap());
}

#[test]
fn absolute_and_low_setting_share_one_first_record_and_never_send_59_signal() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = preview_alert(&io, &fixture.0).identity.unwrap();
    absolute(&io, &fixture.0, &identity, 42, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().levels, [42, 42]);
    alert_low(&io, &fixture.0, &identity, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().levels, [0, 0]);
    assert_eq!(io.0.borrow().cache, [42, 20]);
    assert!(io.0.borrow().signals.is_empty());
    let revision = preview_alert(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 45]);
    assert_eq!(io.0.borrow().cache, [0, 23]);
}
#[test]
fn absolute_does_not_require_low_control_and_unknown_prior_cache_is_captured_exactly() {
    let io = Memory::new();
    let fixture = Fixture::new();
    io.0.borrow_mut().rule.feature = false;
    io.0.borrow_mut().cache = [68, 12];
    assert!(preview(&io, &fixture.0).identity.is_none());
    let identity = preview_alert(&io, &fixture.0).identity.unwrap();
    absolute(&io, &fixture.0, &identity, 0, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().cache, [68, 12]);
    let revision = preview_alert(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 45]);
    assert_eq!(io.0.borrow().cache, [68, 12]);
}
#[test]
fn confirmed_policy_can_naturally_end_existing_cache_without_guessing_an_earlier_baseline() {
    let io = Memory::new();
    let fixture = Fixture::new();
    io.0.borrow_mut().levels = [0, 32];
    io.0.borrow_mut().cache = [68, 12];
    io.0.borrow_mut().power.ac_online = Some(true);
    let identity = fixture.identity(&io);
    assert!(
        check(&io, &fixture.0, &identity, &mut || Ok(()))
            .unwrap()
            .is_some()
    );
    assert_eq!(io.0.borrow().levels, [68, 68]);
    assert_eq!(io.0.borrow().cache, [0, 12]);
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [0, 32]);
    assert_eq!(io.0.borrow().cache, [68, 12]);
}
