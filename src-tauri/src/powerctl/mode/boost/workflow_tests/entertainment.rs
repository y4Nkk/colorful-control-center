use super::*;

fn reviewed_entertainment(f: &Fixture) {
    f.hal
        .with_dchu(|io| {
            io.test_entertainment_options
                .store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    let mut state = f.device.0.lock().unwrap();
    state.bios[10] |= 4;
    state.pages[1][1] = 3;
    state.pages[1][16] = 3;
}

#[test]
fn reviewed_entertainment_applies_and_releases_both_flags_in_place() {
    let f = setup();
    reviewed_entertainment(&f);
    let _gate = f.hal.begin_write().unwrap();
    assert!(read(&f.hal).unwrap().applies_in_current_mode);
    for (turbo, dtt, command) in [
        (true, false, 67),
        (true, true, 195),
        (false, true, 131),
        (false, false, 3),
    ] {
        f.device.0.lock().unwrap().events.clear();
        apply(&f.hal, &request(&f, turbo, dtt)).unwrap();
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.pages[1][1], 3);
        assert_eq!(state.pages[1][16], 3);
        assert!(
            state
                .events
                .iter()
                .any(|event| event == &format!("MODE:{command}"))
        );
        if matches!(command, 195 | 3) {
            assert!(!state.events.iter().any(|event| event == "DCHU:121"));
        }
    }
}

#[test]
fn turbo_displaces_entertainment_noiseless_fan() {
    let f = setup();
    reviewed_entertainment(&f);
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[20] |= 1;
        state.pages[4][9] = 1;
        state.fan_mode = FAN_MODE_NOISELESS;
    }
    let _gate = f.hal.begin_write().unwrap();
    assert!(read(&f.hal).unwrap().quiet_fan.unwrap().active);
    apply(&f.hal, &request(&f, true, false)).unwrap();
    let active = read(&f.hal).unwrap();
    assert!(!active.quiet_fan.unwrap().enabled);
    assert_eq!(f.device.0.lock().unwrap().fan_command, Some(FAN_MODE_AUTO));
}

#[test]
fn entertainment_rejects_unavailable_dtt_driver_without_blocking_turbo() {
    let mut f = setup();
    reviewed_entertainment(&f);
    f.hal.set_test_dtt_driver(std::sync::Arc::new(|| Ok(false)));
    let _gate = f.hal.begin_write().unwrap();
    assert!(apply(&f.hal, &request(&f, false, true)).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    apply(&f.hal, &request(&f, true, false)).unwrap();
    assert!(
        f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event == "MODE:67")
    );
}

#[test]
fn deferred_entertainment_preferences_do_not_become_the_initial_command() {
    let f = setup();
    reviewed_entertainment(&f);
    f.device.0.lock().unwrap().pages[1][32] = 1;
    let _gate = f.hal.begin_write().unwrap();
    let saved = read(&f.hal).unwrap();
    assert_eq!(saved.current.dtt, Some(true));
    assert!(!saved.dtt_active);
    apply(&f.hal, &request(&f, false, true)).unwrap();
    let active = read(&f.hal).unwrap();
    assert!(active.dtt_active);
    assert_eq!(f.device.0.lock().unwrap().pages[1][32], 1);
}

#[test]
fn changing_dtt_keeps_pending_turbo_saved_and_preserves_the_current_fan() {
    let f = setup();
    reviewed_entertainment(&f);
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[4][8] = 1;
        state.fan_mode = FAN_MODE_CUSTOM;
    }
    let _gate = f.hal.begin_write().unwrap();
    let mut dtt = request(&f, true, true);
    dtt.values.turbo_fan = None;
    apply(&f.hal, &dtt).unwrap();
    let active = read(&f.hal).unwrap();
    assert!(!active.turbo_active && active.dtt_active);
    assert_eq!(active.current.turbo_fan, Some(true));
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.fan_mode, FAN_MODE_CUSTOM);
    assert!(state.events.iter().any(|event| event == "MODE:131"));
    assert!(!state.events.iter().any(|event| event == "DCHU:121"));
    assert!(
        !state
            .events
            .iter()
            .any(|event| event.starts_with("PAGE4:8:"))
    );
}

#[test]
fn temporary_entertainment_quiet_keeps_the_saved_preference_and_uses_auto_as_base() {
    let f = setup();
    reviewed_entertainment(&f);
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[20] |= 1;
        state.bios[21] |= 1;
    }
    let _gate = f.hal.begin_write().unwrap();
    fanctl::set_mode(&f.hal, FAN_MODE_NOISELESS).unwrap();
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.fan_mode, FAN_MODE_NOISELESS);
        assert_eq!(
            [state.pages[4][5], state.pages[4][9], state.pages[4][73]],
            [0, 0, 0]
        );
    }
    apply(&f.hal, &request(&f, true, false)).unwrap();
    let mut quiet = request(&f, false, false);
    quiet.values.dtt = None;
    quiet.quiet_fan = Some(QuietTarget::Current);
    apply(&f.hal, &quiet).unwrap();
    let profile = read(&f.hal).unwrap();
    assert!(!profile.turbo_active);
    let quiet = profile.quiet_fan.unwrap();
    assert!(quiet.active && !quiet.enabled);
}

#[test]
fn reviewed_firmware_still_requires_live_option_capabilities() {
    let f = setup();
    reviewed_entertainment(&f);
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[10] &= !4;
        state.bios[16] &= !16;
        state.bios[11] &= !16;
    }
    let _gate = f.hal.begin_write().unwrap();
    let options = read(&f.hal).unwrap();
    assert!(!options.applies_in_current_mode);
    assert!(options.current.turbo_fan.is_none() && options.current.dtt.is_none());
    assert!(apply(&f.hal, &request(&f, true, true)).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn local_firmware_packet_opens_both_controls_in_both_modes() {
    let f = setup();
    reviewed_entertainment(&f);
    let live_packet = [
        0x01, 0x00, 0x25, 0x00, 0x68, 0x04, 0x53, 0x00, 0x02, 0x70, 0x1c, 0x02, 0x00, 0xd3, 0x08,
        0x00, 0x00, 0x00, 0x07, 0x00, 0x01, 0x00, 0x00,
    ];
    f.device.0.lock().unwrap().bios[..live_packet.len()].copy_from_slice(&live_packet);
    let _gate = f.hal.begin_write().unwrap();
    for mode in [2, 3] {
        f.device.0.lock().unwrap().pages[1][1] = mode;
        let opts = read(&f.hal).unwrap();
        assert_eq!(opts.current_mode, mode);
        assert_eq!(
            (opts.current.turbo_fan, opts.current.dtt),
            (Some(false), Some(false))
        );
        assert_eq!(opts.dtt_driver_available, Some(true));
        assert!(opts.applies_in_current_mode);
    }
    apply(&f.hal, &request(&f, true, true)).unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pages[1][1], 3);
    assert!(state.events.iter().any(|event| event == "MODE:195"));
}

#[test]
fn entertainment_requires_confirmed_custom_to_auto_turbo_handoff() {
    let f = setup();
    reviewed_entertainment(&f);
    f.device.0.lock().unwrap().fan_mode = FAN_MODE_CUSTOM;
    let _gate = f.hal.begin_write().unwrap();
    let mut turbo = request(&f, true, false);
    assert!(apply(&f.hal, &turbo).is_err());
    turbo.replace_custom_fan = true;
    turbo.values.dtt = Some(true);
    assert!(apply(&f.hal, &turbo).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    turbo.values.dtt = Some(false);
    apply(&f.hal, &turbo).unwrap();
    {
        let state = f.device.0.lock().unwrap();
        let auto = state
            .events
            .iter()
            .position(|event| event == "DCHU:121")
            .unwrap();
        let command = state
            .events
            .iter()
            .position(|event| event == "MODE:67")
            .unwrap();
        assert!(auto < command);
        assert_eq!(state.pages[4][5], FAN_MODE_AUTO);
    }
}
