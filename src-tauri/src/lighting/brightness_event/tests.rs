use super::*;
use crate::lighting::tests::{io, preset};

fn info(kind: u8, display_maximum: u8) -> Info {
    Info {
        keyboard_type: kind,
        display_maximum,
        light_bar: false,
    }
}

#[test]
fn rgb_cycle_preserves_saved_brightness_when_off_and_never_replays_mode_or_color() {
    let mut page = preset();
    page[80] = 0;
    page[33..35].copy_from_slice(&[72, 73]);
    page[36..40].copy_from_slice(&[1, 2, 3, 4]);
    page[96..108].copy_from_slice(&[1, 2, 3, 4, 1, 6, 7, 8, 1, 10, 11, 12]);
    page[127] = 9;
    let io = io(page);
    let mut initial = None;
    for level in [1, 2, 3, 4, 0, 1] {
        io.words.lock().unwrap().clear();
        let report = handle(&io, &mut initial, &info(22, 4)).unwrap().unwrap();
        assert_eq!(report.level, level);
        assert_eq!(report.display_maximum, 4);
        assert!(!report.firmware_report);
        let mut expected = page;
        expected[35] = if level == 0 { 4 } else { level };
        expected[80] = u8::from(level != 0);
        assert_eq!(*io.page.lock().unwrap(), expected);
        let words = io.words.lock().unwrap();
        assert_eq!(words.len(), if level == 0 { 1 } else { 2 });
        assert!(
            words
                .iter()
                .all(|(cmd, word)| *cmd == 103 && matches!(word >> 24, 0xf4 | 0xe0))
        );
        if level != 0 {
            assert_eq!(
                words[0],
                (103, encode_rgb_brightness_word(level, Some(22)).unwrap())
            );
        }
    }
    assert!(transaction::restore_initial(&io, &initial).ok);
    assert_eq!(*io.page.lock().unwrap(), page);
}

#[test]
fn active_zero_has_no_original_cycle_transition_and_does_not_create_baseline() {
    let mut page = preset();
    page[35] = 0;
    let io = io(page);
    let mut initial = None;
    assert!(handle(&io, &mut initial, &info(22, 4)).unwrap().is_none());
    assert!(initial.is_none());
    assert!(io.words.lock().unwrap().is_empty());
    assert_eq!(io.saved_writes.get(), 0);
}

#[test]
fn invalid_types_levels_and_changed_shared_capabilities_never_write() {
    for kind in [0, 2, 6, 23, 38, 3, 255] {
        let io = io(preset());
        assert!(handle(&io, &mut None, &info(kind, 4)).is_err());
        assert!(io.words.lock().unwrap().is_empty());
        assert_eq!(io.saved_writes.get(), 0);
    }
    let mut page = preset();
    page[35] = 5;
    let io = io(page);
    assert!(handle(&io, &mut None, &info(22, 4)).is_err());
    assert!(io.words.lock().unwrap().is_empty());
    let io = crate::lighting::tests::io(preset());
    let mut changed = info(22, 4);
    changed.light_bar = true;
    assert!(handle(&io, &mut None, &changed).is_err());
    assert!(io.words.lock().unwrap().is_empty());
}

#[test]
fn every_brightness_command_failure_restores_current_state_and_keeps_initial_retry() {
    for stage in [0, 1] {
        let page = preset();
        let io = io(page);
        let mut initial = None;
        io.page.lock().unwrap()[35] = 1;
        let before = *io.page.lock().unwrap();
        io.fail_at.set(Some(stage));
        assert!(handle(&io, &mut initial, &info(22, 4)).is_err());
        assert_eq!(*io.page.lock().unwrap(), before);
        assert!(initial.is_some());
        assert!(transaction::restore_initial(&io, &initial).ok);
    }
}

#[test]
fn both_saved_field_failures_and_ignored_write_roll_back_without_touching_other_fields() {
    for (failure, ignored) in [(Some(0), None), (Some(1), None), (None, Some(0))] {
        let mut page = preset();
        page[80] = 0;
        let io = io(page);
        let mut initial = None;
        io.fail_saved_at.set(failure);
        io.ignore_saved_at.set(ignored);
        assert!(handle(&io, &mut initial, &info(22, 4)).is_err());
        assert_eq!(*io.page.lock().unwrap(), page);
        assert!(initial.is_some());
        assert!(
            io.words
                .lock()
                .unwrap()
                .iter()
                .all(|(_, word)| matches!(word >> 24, 0xf4 | 0xe0))
        );
    }
}

#[test]
fn failed_immediate_compensation_preserves_retry_and_still_restores_saved_fields() {
    let page = preset();
    let io = io(page);
    let mut initial = None;
    io.fail_all_raw.set(true);
    assert!(handle(&io, &mut initial, &info(22, 4)).is_err());
    assert!(initial.is_some());
    assert_eq!(*io.page.lock().unwrap(), page);
    assert!(io.saved_writes.get() > 0);
    io.fail_all_raw.set(false);
    assert!(transaction::restore_initial(&io, &initial).ok);
}

#[test]
fn white_reports_synchronize_only_two_cache_fields_without_hardware_or_baseline() {
    for display_maximum in [2, 5] {
        for level in 0..=display_maximum {
            let mut page = preset();
            page[32] = 255;
            page[35] = 61;
            page[80] = 72;
            let io = io(page);
            let mut initial = None;
            io.white_report.set(Some(i32::from(level)));
            let report = handle(&io, &mut initial, &info(1, display_maximum))
                .unwrap()
                .unwrap();
            assert!(report.firmware_report);
            assert_eq!(report.level, level);
            assert_eq!(report.display_maximum, display_maximum);
            let mut expected = page;
            expected[35] = level;
            expected[80] = u8::from(level != 0);
            assert_eq!(*io.page.lock().unwrap(), expected);
            assert!(io.words.lock().unwrap().is_empty());
            assert!(initial.is_none());
        }
    }
}

#[test]
fn ordinary_white_display_style_keeps_valid_higher_reports_without_desktop_prompt() {
    for level in 3..=5 {
        let io = io(preset());
        io.white_report.set(Some(level));
        let report = handle(&io, &mut None, &info(1, 2)).unwrap().unwrap();
        assert_eq!(report.level, level as u8);
        assert_eq!(report.display_maximum, 2);
        assert!(!report.show_osd());
        assert_eq!(io.page.lock().unwrap()[35], level as u8);
        assert!(io.words.lock().unwrap().is_empty());
    }
}

#[test]
fn white_missing_echo_out_of_range_and_negative_reports_never_change_cache() {
    for (value, display_maximum) in [(None, 5), (Some(61), 5), (Some(-1), 5), (Some(6), 5)] {
        let page = preset();
        let io = io(page);
        io.white_report.set(value);
        let mut initial = None;
        assert!(handle(&io, &mut initial, &info(1, display_maximum)).is_err());
        assert_eq!(*io.page.lock().unwrap(), page);
        assert_eq!(io.saved_writes.get(), 0);
        assert!(io.words.lock().unwrap().is_empty());
        assert!(initial.is_none());
    }
}

#[test]
fn failed_white_report_save_restores_cache_without_any_brightness_command() {
    for failure in [0, 1] {
        let mut page = preset();
        page[80] = 0;
        page[35] = 4;
        let io = io(page);
        io.white_report.set(Some(2));
        io.fail_saved_at.set(Some(failure));
        assert!(handle(&io, &mut None, &info(1, 5)).is_err());
        assert_eq!(*io.page.lock().unwrap(), page);
        assert!(io.words.lock().unwrap().is_empty());
    }
}

#[test]
fn white_report_keeps_existing_manual_baseline_and_same_report_needs_no_save() {
    let io = io(preset());
    let mut initial = Some(
        transaction::capture(&io, KeyboardFamily::White, Some(1))
            .unwrap_or_else(|e| panic!("{}", e.message)),
    );
    let old = initial.as_ref().unwrap().block.clone();
    io.white_report.set(Some(2));
    handle(&io, &mut initial, &info(1, 5)).unwrap();
    assert_eq!(initial.as_ref().unwrap().block, old);
    let writes = io.saved_writes.get();
    handle(&io, &mut initial, &info(1, 5)).unwrap();
    assert_eq!(io.saved_writes.get(), writes);
    assert!(io.words.lock().unwrap().is_empty());
}

#[test]
fn exact_six_level_flag_and_capability_changes_bind_authorization() {
    let mut flags = [0; 10];
    flags[0] = 1;
    assert_eq!(white_display_maximum(&flags).unwrap(), 2);
    flags[7] = 64;
    assert_eq!(white_display_maximum(&flags).unwrap(), 5);
    flags[7] = 32;
    assert_eq!(white_display_maximum(&flags).unwrap(), 2);
    flags[0] = 0;
    assert!(white_display_maximum(&flags).is_err());
    assert!(white_display_maximum(&[]).is_err());
    assert_ne!(
        info(1, 2).identity().unwrap(),
        info(1, 5).identity().unwrap()
    );
    let mut shared = info(22, 4);
    shared.light_bar = true;
    assert_ne!(info(22, 4).identity().unwrap(), shared.identity().unwrap());
}

#[test]
fn capability_changed_during_failed_output_preserves_baseline_until_safe_retry() {
    let mut page = preset();
    page[35] = 1;
    let io = io(page);
    let mut initial = None;
    io.fail_at.set(Some(1));
    io.change_cap_at.set(Some(1));
    assert!(handle(&io, &mut initial, &info(22, 4)).is_err());
    assert_eq!(*io.page.lock().unwrap(), page);
    assert!(initial.is_some());
    let count = io.words.lock().unwrap().len();
    assert!(!transaction::restore_initial(&io, &initial).ok);
    assert_eq!(io.words.lock().unwrap().len(), count);
    io.light_bar.set(false);
    assert!(transaction::restore_initial(&io, &initial).ok);
}
