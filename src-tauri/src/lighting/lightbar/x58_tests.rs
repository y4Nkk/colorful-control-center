use super::*;
use crate::lighting::lightbar::x58;

fn original() -> Snapshot {
    let mut bytes = [77; 19];
    bytes[..5].copy_from_slice(&[0, 3, 4, 2, 6]);
    Snapshot {
        kind: Kind::X58,
        identity: "isolated-x58".into(),
        bytes,
    }
}
fn memory() -> Memory {
    let io = Memory::new();
    *io.current.borrow_mut() = original();
    io
}
fn request(io: &Memory, settings: x58::Values) -> Request {
    Request {
        expected_revision: io.current.borrow().revision(),
        values: Values::X58(settings),
    }
}
fn settings(effect: x58::Effect) -> x58::Values {
    x58::for_effect(&original().bytes, effect)
}

#[test]
fn six_modes_seven_colors_and_four_byte_commands_match_original() {
    let before = original();
    for (index, effect) in x58::EFFECTS.into_iter().enumerate() {
        let target = before.target(&Values::X58(settings(effect))).unwrap();
        let packets = target.packets().unwrap();
        assert_eq!(&packets[0][..4], &[0xcd, 0xe2, 1, 3]);
        assert_eq!(&packets[1][..4], &[0xcd, 0xe2, index as u8 + 5, 0]);
        assert_eq!(packets.last().unwrap()[..4], [0xcd, 0xe2, 2, 5]);
        if index == 0 || index == 4 {
            assert_eq!(packets.len(), 4);
            assert_eq!(
                &packets[2][..4],
                &[0xcd, 0xe2, 3, if index == 0 { 2 } else { 6 }]
            );
        } else {
            assert_eq!(packets.len(), 3);
        }
        assert!(
            packets
                .iter()
                .all(|packet| packet[4..].iter().all(|value| *value == 0))
        );
    }
    for id in 1..=7 {
        let mut value = settings(x58::Effect::Static);
        value.color_id = Some(id);
        assert_eq!(
            before
                .target(&Values::X58(value))
                .unwrap()
                .packets()
                .unwrap()[2][3],
            id
        );
    }
}
#[test]
fn brightness_zero_remains_original_level_one_without_a_fabricated_off_packet() {
    let before = original();
    let mut value = settings(x58::Effect::Breath);
    value.brightness = 0;
    let target = before.target(&Values::X58(value)).unwrap();
    let packets = target.packets().unwrap();
    assert_eq!(target.bytes[2], 0);
    assert_eq!(&packets.last().unwrap()[..4], &[0xcd, 0xe2, 2, 1]);
    assert!(
        !packets
            .iter()
            .any(|packet| packet[0] == 0xcc || packet[1] == 191)
    );
}
#[test]
fn color_targets_preserve_inactive_color_and_all_reserved_bytes() {
    let before = original();
    let mut value = settings(x58::Effect::Scan);
    value.color_id = Some(7);
    let after = before.target(&Values::X58(value)).unwrap();
    assert_eq!(after.bytes[3], 2);
    assert_eq!(after.bytes[4], 7);
    assert_eq!(&after.bytes[5..], &before.bytes[5..]);
    let after = after
        .target(&Values::X58(settings(x58::Effect::Wave)))
        .unwrap();
    assert_eq!(&after.bytes[3..5], &[2, 7]);
}
#[test]
fn every_send_and_partial_save_failure_compensates_x58_packets_and_preserves_first_state() {
    for failure in 1..=5 {
        let io = memory();
        let request = request(&io, settings(x58::Effect::Scan));
        let mut baseline = None;
        if failure <= 4 {
            io.fail_send_at.set(failure);
        } else {
            io.fail_save.set(true);
        }
        assert!(apply(&io, &mut baseline, &request).is_err());
        assert_eq!(io.current.borrow().bytes, original().bytes);
        assert_eq!(baseline.unwrap().kind, Kind::X58);
        assert_eq!(&io.sends.borrow().last().unwrap()[..4], &[0xcd, 0xe2, 2, 5]);
    }
}
#[test]
fn failed_restore_still_attempts_other_packets_and_saved_original_then_retries() {
    let io = memory();
    let mut baseline = None;
    let mut value = settings(x58::Effect::Breath);
    value.brightness = 1;
    apply(&io, &mut baseline, &request(&io, value)).unwrap();
    let before_attempts = io.attempts.get();
    io.fail_send_at.set(before_attempts + 1);
    let revision = io.current.borrow().revision();
    assert!(restore_initial(&io, &baseline, &revision).is_err());
    assert_eq!(io.attempts.get(), before_attempts + 4);
    assert_eq!(io.current.borrow().bytes, original().bytes);
    let revision = io.current.borrow().revision();
    restore_initial(&io, &baseline, &revision).unwrap();
}
#[test]
fn kind_and_identity_are_part_of_review_and_a_different_kind_cannot_reuse_baseline() {
    let io = memory();
    let mut initial = original();
    initial.kind = Kind::X170;
    assert_ne!(initial.revision(), original().revision());
    assert!(!initial.same_device(&original()));
    let request = request(&io, settings(x58::Effect::Wave));
    let mut baseline = Some(initial);
    assert!(apply(&io, &mut baseline, &request).is_err());
    let revision = io.current.borrow().revision();
    assert!(restore_initial(&io, &baseline, &revision).is_err());
    assert!(io.sends.borrow().is_empty());
}
#[test]
fn unknown_current_is_not_initialized_and_known_initial_restores_without_decoding_bad_current() {
    let io = memory();
    let baseline = Some(original());
    io.current.borrow_mut().bytes[0] = 255;
    let state = profile(&io.current.borrow(), &baseline, None).unwrap();
    assert!(state.current.is_none());
    assert_eq!(state.kind, Kind::X58);
    assert!(state.presets.is_empty());
    assert!(apply(&io, &mut None, &request(&io, settings(x58::Effect::Static))).is_err());
    assert_eq!(io.saved.get(), 0);
    assert!(io.sends.borrow().is_empty());
    let revision = io.current.borrow().revision();
    restore_initial(&io, &baseline, &revision).unwrap();
    assert_eq!(io.current.borrow().bytes, original().bytes);
}
#[test]
fn unknown_inactive_colors_are_preserved_and_require_explicit_color_when_that_mode_is_selected() {
    let io = memory();
    {
        let mut current = io.current.borrow_mut();
        current.bytes[0] = 5;
        current.bytes[3..5].fill(0);
    }
    let before = io.current.borrow().clone();
    let state = profile(&before, &None, None).unwrap();
    assert!(state.current.is_some());
    assert_eq!(state.presets.len(), 4);
    let mut value = settings(x58::Effect::Static);
    value.color_id = Some(7);
    let mut baseline = None;
    apply(&io, &mut baseline, &request(&io, value)).unwrap();
    assert_eq!(&io.current.borrow().bytes[3..5], &[7, 0]);
    let revision = io.current.borrow().revision();
    restore_initial(&io, &baseline, &revision).unwrap();
    assert_eq!(io.current.borrow().bytes, before.bytes);
}
#[test]
fn unsupported_fields_invalid_levels_and_legacy_contracts_fail_before_write() {
    let io = memory();
    for (speed, brightness, color) in [
        (0, 4, Some(1)),
        (4, 4, Some(1)),
        (1, 5, Some(1)),
        (1, 1, Some(0)),
        (1, 1, Some(8)),
        (1, 1, None),
    ] {
        let value = x58::Values {
            effect: x58::Effect::Static,
            speed,
            brightness,
            color_id: color,
        };
        assert!(apply(&io, &mut None, &request(&io, value)).is_err());
    }
    let mismatch = Request {
        expected_revision: io.current.borrow().revision(),
        values: initial().values().unwrap(),
    };
    assert!(apply(&io, &mut None, &mismatch).is_err());
    assert!(
        serde_json::from_str::<Values>(
            r#"{"enabled":true,"brightness":3,"speed":1,"effect":"static","color":null}"#
        )
        .is_err()
    );
    assert!(serde_json::from_str::<Values>(r#"{"kind":"x58","settings":{"effect":"static","brightness":3,"speed":1,"colorId":1,"enabled":false}}"#).is_err());
    assert!(io.sends.borrow().is_empty());
    assert_eq!(io.saved.get(), 0);
}
#[test]
fn new_light_bar_capability_is_independent_and_requires_exact_channel_status() {
    let mut flags = [0; 24];
    flags[0] = 1;
    flags[23] = 3 | 64;
    assert!(model::kinds(&flags).unwrap().is_empty());
    flags[23] |= 128;
    assert_eq!(model::kinds(&flags).unwrap(), vec![Kind::X58]);
    flags[11] = 8;
    assert_eq!(model::kinds(&flags).unwrap(), vec![Kind::X170, Kind::X58]);
    assert!(x58::supported(1).unwrap());
    assert!(!x58::supported(0).unwrap());
    assert!(x58::supported(60).is_err());
    assert_eq!((Kind::X58.product(), Kind::X58.report_id()), (0x8911, 0xcd));
    assert_eq!(
        (Kind::X170.product(), Kind::X170.report_id()),
        (0x8297, 0xcc)
    );
}
#[test]
fn unchanged_values_do_not_capture_or_write_and_later_failures_keep_original_baseline() {
    let io = memory();
    let mut baseline = None;
    let request_same = request(&io, settings(x58::Effect::Static));
    apply(&io, &mut baseline, &request_same).unwrap();
    assert!(baseline.is_none());
    assert!(io.sends.borrow().is_empty());
    apply(
        &io,
        &mut baseline,
        &request(&io, settings(x58::Effect::Breath)),
    )
    .unwrap();
    io.fail_save.set(true);
    assert!(
        apply(
            &io,
            &mut baseline,
            &request(&io, settings(x58::Effect::Wave))
        )
        .is_err()
    );
    assert_eq!(io.current.borrow().bytes[0], 1);
    assert_eq!(baseline.as_ref().unwrap().bytes, original().bytes);
    let revision = io.current.borrow().revision();
    restore_initial(&io, &baseline, &revision).unwrap();
}
