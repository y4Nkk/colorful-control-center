use super::*;

fn device(enabled: bool, automatic: bool) -> Device {
    let mut device = crate::gpu_power::test_fixture::EnvironmentFixture::new()
        .io
        .device();
    device.enabled = enabled;
    device.problem = if enabled { 0 } else { 22 };
    device.automatic_power_off = automatic;
    device
}
fn target(before: &Device, enabled: bool, automatic_power_off: bool) -> Target {
    Target {
        id: before.id.clone(),
        name: before.name.clone(),
        values: model::Target {
            enabled,
            automatic_power_off,
        },
    }
}

#[test]
fn closed_gpu_saving_mode_and_final_enable_share_three_continuous_frozen_stages() {
    let before = device(false, true);
    let wanted = target(&before, true, false);
    let stages = Stages::build(&before, Some(1), Some(&wanted), true).unwrap();
    let requests = stages.requests();
    assert_eq!(requests.len(), 3);
    assert!(stages.preparation.is_some() && stages.after_mode.is_some() && stages.finish.is_some());
    assert!(stages.before_mode.is_none());
    assert_eq!(
        requests
            .iter()
            .map(|request| request.target.enabled)
            .collect::<Vec<_>>(),
        vec![true, false, true]
    );
    assert_eq!(
        requests
            .iter()
            .map(|request| request.target.automatic_power_off)
            .collect::<Vec<_>>(),
        vec![true, true, false]
    );
    assert_eq!(stages.before, before);
    assert!(stages.mode_before().enabled);
    assert!(!stages.after_mode.as_ref().unwrap().target.enabled);
    assert_eq!(stages.final_device().target(), wanted.values);
    sequence::Sequence::new(&requests).unwrap();
}

#[test]
fn stage_graph_covers_all_modes_supplies_independent_targets_and_dependency_needs() {
    for enabled in [false, true] {
        for automatic in [false, true] {
            for mode in [None, Some(0), Some(1), Some(2), Some(3)] {
                for wanted in [
                    None,
                    Some((true, false)),
                    Some((true, true)),
                    Some((false, false)),
                    Some((false, true)),
                ] {
                    for needs_gpu in [false, true] {
                        let before = device(enabled, automatic);
                        let target =
                            wanted.map(|(enabled, automatic)| target(&before, enabled, automatic));
                        let result = Stages::build(&before, mode, target.as_ref(), needs_gpu);
                        if needs_gpu && wanted.is_some_and(|(enabled, _)| !enabled) {
                            assert!(result.is_err());
                            continue;
                        }
                        let stages = result.unwrap();
                        assert!(stages.requests().len() <= 3);
                        if !stages.requests().is_empty() {
                            sequence::Sequence::new(&stages.requests()).unwrap();
                        }
                        let expected = target.map(|target| target.values).unwrap_or_else(|| {
                            if needs_gpu {
                                model::Target {
                                    enabled: true,
                                    automatic_power_off: automatic,
                                }
                            } else {
                                mode.and_then(|mode| {
                                    super::mode::action(true, Some(&before), mode, None, false)
                                        .unwrap()
                                })
                                .map(|action| action.target)
                                .unwrap_or(before.target())
                            }
                        });
                        assert_eq!(stages.final_device().target(), expected);
                        assert_eq!(stages.before, before);
                    }
                }
            }
        }
    }
}

#[test]
fn projected_stages_preserve_display_qualification_and_reject_wrong_devices() {
    let mut before = device(false, true);
    before.drives_display = true;
    before.can_disable = false;
    assert!(Stages::build(&before, Some(1), None, true).is_err());
    before.drives_display = false;
    before.can_disable = false;
    assert!(Stages::build(&before, Some(1), None, true).is_err());
    before.can_disable = true;
    let mut wanted = target(&before, true, false);
    wanted.id.push_str("_OTHER");
    assert!(Stages::build(&before, Some(3), Some(&wanted), true).is_err());
}

#[test]
fn graph_requires_every_nullable_field_and_rejects_unknown_old_fields() {
    let value =
        serde_json::to_value(Stages::build(&device(true, false), None, None, false).unwrap())
            .unwrap();
    for field in ["mode", "preparation", "beforeMode", "afterMode", "finish"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Stages>(missing).is_err());
    }
    let mut old = value;
    old["singleRequest"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Stages>(old).is_err());
}
