use super::*;

#[test]
fn unsupported_models_are_read_only_and_cannot_fall_through_to_generic_mode_writes() {
    let f = Fixture::new();
    enable_policy(&f);
    let before = {
        let mut state = f.device.0.lock().unwrap();
        state.bios[16] |= 16;
        state.events.clone()
    };
    let _gate = f.hal.begin_write().unwrap();
    for low in [0xf0, 0xf4] {
        f.device.0.lock().unwrap().pages[0][2..6].copy_from_slice(&[0x50, low, 0, 0]);
        let profile = powerctl::mode::read(&f.hal).unwrap();
        assert!(profile.available_modes.is_empty() && profile.targets.is_empty());
        assert!(profile.blocked_reason.unwrap().contains("尚未支持"));
        let error = powerctl::mode::checkpoint(&f.hal, 2).err().unwrap();
        assert!(error.to_string().contains("尚未支持"));
        assert!(
            powerctl::mode::boost::capture_values(&f.hal)
                .unwrap()
                .is_none()
        );
        let options = powerctl::mode::boost::read(&f.hal).unwrap();
        assert!(options.blocked_reason.unwrap().contains("尚未支持"));
        for enabled in [false, true] {
            let request = powerctl::mode::boost::Request {
                expected_revision: options.revision.clone(),
                quiet_fan: None,
                replace_custom_fan: false,
                values: powerctl::mode::boost::Values {
                    turbo_fan: Some(enabled),
                    dtt: None,
                },
            };
            assert!(
                powerctl::mode::boost::apply(&f.hal, &request)
                    .unwrap_err()
                    .to_string()
                    .contains("尚未支持")
            );
        }
    }
    assert_eq!(f.device.0.lock().unwrap().events, before);
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}

#[test]
fn repeated_modes_keep_every_touched_plan_at_its_first_value_in_the_shared_baseline() {
    let f = Fixture::new();
    enable_policy(&f);
    let initial = f.device.0.lock().unwrap().plan_values.clone();
    let _gate = f.hal.begin_write().unwrap();
    for mode in [4, 2, 4] {
        powerctl::mode::checkpoint(&f.hal, mode)
            .unwrap()
            .execute(&f.hal)
            .unwrap();
    }
    let baseline = f.hal.recovery().unwrap().oem_mode.clone().unwrap();
    let values = baseline.stored().system_plan_values().unwrap();
    assert!(!values.is_empty());
    for (scheme, value) in &values {
        let key = u128::from_str_radix(scheme, 16).unwrap();
        assert_eq!(initial.get(&key), Some(value), "{scheme}");
    }
}
