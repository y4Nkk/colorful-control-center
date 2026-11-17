use super::*;

#[test]
fn type38_exact_levels_are_distinct_from_other_keyboard_and_cover_values() {
    assert_eq!(family_from_type(38), KeyboardFamily::ZoneRgb);
    for (level, raw) in [47, 47, 102, 155, 212].into_iter().enumerate() {
        assert_eq!(
            encode_rgb_brightness_word(level as u8, Some(38)).unwrap(),
            0xF400_0000 | raw
        );
    }
    assert_eq!(encode_rgb_brightness_word(3, Some(2)).unwrap(), 0xF400_008F);
    assert_eq!(encode_rgb_brightness_word(3, None).unwrap(), 0xF400_008F);
    assert!(encode_rgb_brightness_word(5, Some(38)).is_err());
}
#[test]
fn type38_active_and_disabled_targets_restore_exact_original_fields() {
    for enabled in [true, false] {
        let original = preset();
        let io = io(original);
        let target = Target {
            keyboard_type: 38,
            enabled,
            settings: rgb(Effect::Zones),
        };
        let mut baseline = None;
        assert!(transaction::apply_target(&io, &mut baseline, &target, &mut false).is_ok());
        let sent = words(&io);
        assert_eq!(sent[0], 0xF400_0000 | if enabled { 155 } else { 47 });
        assert_eq!(sent[1], 0xF064_00FF);
        assert_eq!(io.page.lock().unwrap()[35], 3);
        assert_eq!(io.page.lock().unwrap()[80], u8::from(enabled));
        if !enabled {
            assert_eq!(*sent.last().unwrap(), 0xE000_0007);
        }
        assert!(transaction::restore_initial(&io, &baseline).ok);
        assert_eq!(*io.page.lock().unwrap(), original);
        assert!(words(&io).contains(&0xF400_00D4));
    }
}
#[test]
fn type38_partial_send_and_save_failures_keep_first_baseline_and_preserve_shared_page() {
    let probe = io(preset());
    assert!(
        LightingController::default()
            .apply_io(&probe, &mut None, &rgb(Effect::Zones), Some(38))
            .ok
    );
    let count = words(&probe).len();
    for failed in 0..=count {
        let original = preset();
        let io = io(original);
        if failed == count {
            io.fail_saved_at.set(Some(0));
        } else {
            io.fail_at.set(Some(failed));
        }
        let mut baseline = None;
        assert!(
            !LightingController::default()
                .apply_io(&io, &mut baseline, &rgb(Effect::Zones), Some(38))
                .ok
        );
        assert_eq!(*io.page.lock().unwrap(), original);
        assert!(baseline.is_some());
        assert!(transaction::restore_initial(&io, &baseline).ok);
    }
}
