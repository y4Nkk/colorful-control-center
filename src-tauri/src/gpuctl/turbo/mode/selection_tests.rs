use super::*;
use crate::powerctl::mode::links::{Review, Target};

#[test]
fn selection_checks_only_the_confirmed_mode_and_actual_supply_without_fake_future_conditions() {
    let (memory, gpu, _environment, power) = setup();
    {
        let mut data = memory.0.lock().unwrap();
        data.source.saved[0].core_mhz = 400;
        data.source.on_ac = Some(false);
    }
    let hal = gpu_power::test_fixture::hal();
    let context = Context::with_source(&hal, &gpu, &power, Box::new(SourceFixture(memory.clone())));
    let review = context.selection(2);
    let target = review.target().unwrap();
    assert_eq!(target.on_ac, Some(false));
    assert!(target.clocks.driver.is_some());
    assert_eq!(
        target.clocks.candidate.projected.as_ref().unwrap().offsets[0].core_khz,
        Some(0)
    );
    assert!(matches!(
        target.effects().unwrap().clock_before,
        ClockObservation::Ready(_)
    ));
    let ac = context.selection(2);
    assert_eq!(ac, review);
    memory.0.lock().unwrap().source.on_ac = Some(true);
    let ac = context.selection(2);
    assert!(ac.unavailable_reason().is_some());
    assert!(ac.target().is_err());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert_eq!(memory.0.lock().unwrap().source.mode, 2);
}

#[test]
fn selection_offline_preview_defers_driver_and_preserves_both_original_node_preferences() {
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    gpu.set_power_suspended(true).unwrap();
    let hal = gpu_power::test_fixture::hal();
    let context = Context::with_source(&hal, &gpu, &power, Box::new(SourceFixture(memory.clone())));
    let review = context.selection(3);
    let target = review.target().unwrap();
    assert_eq!(
        target.action.unwrap().stage,
        gpu_power::mode::Stage::BeforeMode
    );
    assert!(target.action.unwrap().target.automatic_power_off);
    assert!(target.clocks.driver.is_none() && target.clocks.deferred_reason.is_some());
    assert_eq!(
        target.effects().unwrap().clock_before,
        ClockObservation::PoweredOff
    );
    let quiet = context.selection(0);
    assert_eq!(
        quiet.target().unwrap().action.unwrap().stage,
        gpu_power::mode::Stage::AfterMode
    );
    assert_eq!(memory.0.lock().unwrap().reads, 0);
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 0);
    assert!(!environment.io.device().enabled);
}

#[test]
fn selection_unknown_supply_and_replaced_device_cannot_become_executable_targets() {
    let (memory, gpu, _environment, power) = setup();
    let hal = gpu_power::test_fixture::hal();
    let context = Context::with_source(&hal, &gpu, &power, Box::new(SourceFixture(memory.clone())));
    memory.0.lock().unwrap().source.on_ac = None;
    let unknown = context.selection(3);
    assert!(unknown.unavailable_reason().is_some());
    assert!(unknown.target().is_err());
    assert_eq!(memory.0.lock().unwrap().reads, 0);
    memory.0.lock().unwrap().source.on_ac = Some(true);
    memory.0.lock().unwrap().devices[0].extended += 1;
    // 来源缓存仍指向原来的物理设备。
    struct OldIdentity(Arc<Memory>);
    impl SourceIo for OldIdentity {
        fn read(&self) -> Result<Source> {
            self.0.read()
        }
    }
    impl crate::powerctl::mode::links::Source for OldIdentity {
        fn policy(&self) -> Result<Policy> {
            let mut value = policy(&self.0);
            value.cached_device_id -= 1;
            Ok(value)
        }
        fn on_ac(&self) -> Result<Option<bool>> {
            Ok(Some(true))
        }
    }
    let changed = Context::with_source(&hal, &gpu, &power, Box::new(OldIdentity(memory.clone())))
        .selection(3);
    assert!(changed.unavailable_reason().is_some());
    assert!(changed.target().is_err());
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn selection_node_qualification_is_mode_specific_and_all_nullable_fields_are_required() {
    let (memory, gpu, environment, power) = setup();
    {
        let mut data = environment.io.0.lock().unwrap();
        data.device.automatic_power_off = true;
        data.device.drives_display = true;
        data.device.can_disable = false;
    }
    let hal = gpu_power::test_fixture::hal();
    let context = Context::with_source(&hal, &gpu, &power, Box::new(SourceFixture(memory.clone())));
    assert!(matches!(context.selection(1), Review::Unavailable(_)));
    let review = context.selection(3);
    let target = review.target().unwrap();
    let json = serde_json::to_value(target).unwrap();
    assert_eq!(
        serde_json::from_value::<Target>(json.clone()).unwrap(),
        *target
    );
    for field in ["node", "action", "onAc"] {
        let mut missing = json.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Target>(missing).is_err());
    }
    for field in ["driver", "driverError", "deferredReason"] {
        let mut missing = json.clone();
        missing["clocks"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Target>(missing).is_err());
    }
    let mut missing = json;
    missing["clocks"]["candidate"]
        .as_object_mut()
        .unwrap()
        .remove("projected");
    assert!(serde_json::from_value::<Target>(missing).is_err());
    assert!(memory.0.lock().unwrap().writes.is_empty());
}
