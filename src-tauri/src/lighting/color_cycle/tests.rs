use super::*;
use crate::lighting::tests::{io, preset};

fn info(raw_type: u8) -> Info {
    Info {
        keyboard_type: raw_type,
        customer_id: 1,
        monster_manufacturer: false,
        logo: false,
        light_bar: false,
    }
}
#[test]
fn exact_original_palette_and_wrap_keep_zero_as_initial_position() {
    let expected = [
        [0, 0, 200],
        [0, 255, 0],
        [255, 0, 0],
        [0, 255, 200],
        [0, 127, 200],
        [0, 255, 127],
        [255, 0, 200],
        [127, 0, 200],
        [255, 0, 127],
        [255, 255, 0],
        [127, 255, 200],
        [255, 127, 0],
        [63, 255, 0],
        [255, 127, 63],
        [63, 127, 127],
    ];
    for (current, expected) in expected.into_iter().enumerate() {
        let choice = info(6).next(current as u8).unwrap();
        assert_eq!(choice.index, current as u8 + 1);
        assert_eq!([choice.color.r, choice.color.g, choice.color.b], expected);
    }
    assert_eq!(info(6).next(15).unwrap().index, 1);
    assert!(info(6).next(16).is_err());
    let monster = Info {
        customer_id: 0,
        monster_manufacturer: true,
        ..info(38)
    };
    let last = monster.next(15).unwrap();
    assert_eq!(last.index, 16);
    assert_eq!([last.color.r, last.color.g, last.color.b], [223, 240, 124]);
    assert_eq!(monster.next(16).unwrap().index, 1);
    assert_eq!(monster.next(255).unwrap().index, 0);
    assert_eq!(
        Info {
            customer_id: 1,
            ..monster
        }
        .maximum(),
        15
    );
}
#[test]
fn supported_types_emit_only_original_colors_and_preserve_all_other_settings() {
    for raw_type in [6, 22, 38] {
        for enabled in [false, true] {
            let mut page = preset();
            page[80] = u8::from(enabled);
            page[127] = 5;
            let io = io(page);
            let mut initial = None;
            let choice = apply(&io, &mut initial, &info(raw_type)).unwrap();
            assert_eq!(choice.index, 6);
            let color = Rgb {
                r: 0,
                g: 255,
                b: 127,
            };
            assert_eq!(
                *io.words.lock().unwrap(),
                (0..3)
                    .map(|n| (103, encode_zone_color(0xF0 + n, color, Some(raw_type))))
                    .collect::<Vec<_>>()
            );
            let raw_color = if raw_type == 38 {
                0x0064_00FF
            } else {
                0x0046_00FF
            };
            assert_eq!(
                *io.words.lock().unwrap(),
                (0..3)
                    .map(|zone| (103, ((0xF0 + zone) << 24) | raw_color))
                    .collect::<Vec<_>>()
            );
            let mut expected = page;
            expected[81..84].copy_from_slice(&[0, 255, 127]);
            expected[127] = 6;
            assert_eq!(*io.page.lock().unwrap(), expected);
            assert!(transaction::restore_initial(&io, &initial).ok);
            assert_eq!(*io.page.lock().unwrap(), page);
        }
    }
}
#[test]
fn peripheral_colors_are_replayed_without_being_changed_or_using_f4() {
    let mut page = preset();
    page[105..108].copy_from_slice(&[8, 9, 10]);
    let io = io(page);
    io.cycle_logo.set(true);
    io.light_bar.set(true);
    let info = Info {
        logo: true,
        light_bar: true,
        ..info(22)
    };
    apply(&io, &mut None, &info).unwrap();
    let commands = io.words.lock().unwrap();
    assert_eq!(commands.len(), 5);
    assert_eq!(
        commands[3],
        (
            103,
            encode_zone_color(0xF6, Rgb { r: 4, g: 5, b: 6 }, Some(22))
        )
    );
    assert_eq!(
        commands[4],
        (
            103,
            encode_zone_color(0xF3, Rgb { r: 8, g: 9, b: 10 }, Some(22))
        )
    );
    assert_eq!(&io.page.lock().unwrap()[96..127], &page[96..127]);
}
#[test]
fn each_color_send_and_save_failure_compensates_and_keeps_first_restore() {
    for fail in 0..5 {
        let page = preset();
        let io = io(page);
        if fail < 3 {
            io.fail_at.set(Some(fail));
        } else {
            io.fail_saved_at.set(Some(fail - 3));
        }
        let mut initial = None;
        assert!(
            apply(&io, &mut initial, &info(6)).is_err(),
            "failure {fail}"
        );
        assert_eq!(*io.page.lock().unwrap(), page);
        assert!(initial.is_some());
        assert!(transaction::restore_initial(&io, &initial).ok);
    }
}
#[test]
fn unknown_index_type_or_changed_facts_never_send_or_save() {
    for raw_type in [1, 2, 3, 23, 255] {
        let io = io(preset());
        let mut initial = None;
        assert!(apply(&io, &mut initial, &info(raw_type)).is_err());
        assert!(io.words.lock().unwrap().is_empty());
        assert!(initial.is_none());
    }
    let mut page = preset();
    page[127] = 254;
    let io = io(page);
    let mut initial = None;
    assert!(apply(&io, &mut initial, &info(6)).is_err());
    assert!(io.words.lock().unwrap().is_empty());
    assert!(initial.is_none());
    let io = super::super::tests::io(preset());
    io.cycle_customer.set(0);
    io.cycle_manufacturer.replace("MONSTER".into());
    assert!(apply(&io, &mut None, &info(6)).is_err());
    assert!(io.words.lock().unwrap().is_empty());
}
#[test]
fn later_color_cycles_restore_the_first_index_and_normal_writes_do_not_take_it() {
    let page = preset();
    let io = io(page);
    let mut initial = None;
    assert!(
        transaction::apply_request(
            &io,
            &mut initial,
            &super::super::tests::rgb(Effect::Solid),
            Some(6),
        )
        .is_ok()
    );
    assert!(initial.as_ref().unwrap().cycle_index.is_none());
    io.page.lock().unwrap()[127] = 7;
    apply(&io, &mut initial, &info(6)).unwrap();
    apply(&io, &mut initial, &info(6)).unwrap();
    assert_eq!(io.page.lock().unwrap()[127], 9);
    assert!(transaction::restore_initial(&io, &initial).ok);
    let mut expected = page;
    expected[127] = 7;
    assert_eq!(*io.page.lock().unwrap(), expected);
}

#[test]
fn successful_but_ignored_color_or_index_writes_are_not_reported_as_applied() {
    for step in [0, 1] {
        let page = preset();
        let io = io(page);
        let mut initial = None;
        io.ignore_saved_at.set(Some(step));
        assert!(apply(&io, &mut initial, &info(6)).is_err());
        assert_eq!(*io.page.lock().unwrap(), page);
        assert!(transaction::restore_initial(&io, &initial).ok);
    }
}

#[test]
fn persistent_replay_failure_retains_the_baseline_and_can_be_retried() {
    let page = preset();
    let io = io(page);
    let mut initial = None;
    io.fail_all_raw.set(true);
    let error = apply(&io, &mut initial, &info(6))
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("恢复失败") || error.contains("回放"));
    assert!(initial.is_some());
    assert!(!transaction::restore_initial(&io, &initial).ok);
    io.fail_all_raw.set(false);
    assert!(transaction::restore_initial(&io, &initial).ok);
    assert_eq!(*io.page.lock().unwrap(), page);
}

#[test]
fn conv_u1_cursor_wrap_matches_every_byte_and_restores_original_ff() {
    for monster in [false, true] {
        let rule = Info {
            customer_id: if monster { 0 } else { 1 },
            monster_manufacturer: monster,
            ..info(6)
        };
        for current in 0..=255u8 {
            let next = if current == rule.maximum() {
                1
            } else {
                current.wrapping_add(1)
            };
            match rule.next(current) {
                Ok(choice) => {
                    assert!(next <= rule.maximum());
                    assert_eq!(choice.index, next);
                }
                Err(_) => assert!(next > rule.maximum()),
            }
        }
        let mut page = preset();
        page[127] = 255;
        let io = io(page);
        io.cycle_customer.set(rule.customer_id);
        io.cycle_manufacturer
            .replace(if monster { "MONSTER" } else { "Notebook" }.into());
        let mut initial = None;
        let choice = apply(&io, &mut initial, &rule).unwrap();
        assert_eq!(choice.index, 0);
        assert_eq!(
            [choice.color.r, choice.color.g, choice.color.b],
            [0, 0, 255]
        );
        assert_eq!(io.page.lock().unwrap()[127], 0);
        assert!(transaction::restore_initial(&io, &initial).ok);
        assert_eq!(*io.page.lock().unwrap(), page);
    }
}
