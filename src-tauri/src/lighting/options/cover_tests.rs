use super::*;

fn memory() -> Memory {
    let io = Memory::new();
    {
        let mut caps = io.caps.borrow_mut();
        caps.keyboard_type = 3;
        caps.logo = false;
        caps.light_bar = false;
        caps.top_cover = true;
    }
    io.page.borrow_mut()[100..104].copy_from_slice(&[3, 10, 20, 30]);
    io
}
fn request(io: &Memory) -> Request {
    let snapshot = capture(io).unwrap();
    let mut values = snapshot.values().unwrap();
    values.top_cover = Some(TopCover {
        brightness: 2,
        color: Rgb {
            r: 0,
            g: 254,
            b: 129,
        },
    });
    Request {
        expected_revision: snapshot.revision(),
        values,
    }
}

#[test]
fn exact_active_fnkey_flow_has_three_colors_and_no_unused_f6() {
    let io = memory();
    let request = request(&io);
    transaction::apply(&io, &mut None, &request).unwrap();
    assert_eq!(
        *io.words.borrow(),
        [
            (103, 0xF400005F),
            (103, 0xF04600FE),
            (103, 0xF14600FE),
            (103, 0xF24600FE),
        ]
    );
    assert_eq!(&io.page.borrow()[100..104], [2, 0, 254, 129]);
}
#[test]
fn all_brightness_levels_and_both_special_colors_match_actual_helper() {
    for brightness in 0..=4 {
        for green in [254, 255] {
            let color = Rgb {
                r: 0,
                g: green,
                b: if green == 254 { 129 } else { 127 },
            };
            let words = cover::commands(&TopCover { brightness, color }).unwrap();
            assert_eq!(
                words[0],
                (
                    103,
                    0xF4000000 | [0, 47, 95, 143, 191][usize::from(brightness)]
                )
            );
            assert_eq!(words[1].1, 0xF0460000 | u32::from(green));
        }
    }
    assert!(
        cover::commands(&TopCover {
            brightness: 5,
            color: Rgb { r: 0, g: 0, b: 0 }
        })
        .is_err()
    );
}
#[test]
fn disabled_cover_preserves_color_and_restores_exact_initial_fields() {
    let io = memory();
    let original = io.page.borrow().clone();
    let mut target = request(&io);
    target.values.top_cover.as_mut().unwrap().brightness = 0;
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &target).unwrap();
    assert_eq!(io.words.borrow()[0], (103, 0xF4000000));
    io.page.borrow_mut()[81..84].copy_from_slice(&[77, 88, 99]);
    io.page.borrow_mut()[13] = 123;
    let revision = capture(&io).unwrap().revision();
    transaction::restore(&io, &baseline, &revision).unwrap();
    assert_eq!(&io.page.borrow()[100..104], &original[100..104]);
    assert_eq!(&io.page.borrow()[81..84], [77, 88, 99]);
    assert_eq!(io.page.borrow()[13], 123);
}
#[test]
fn every_command_and_partial_save_failure_recovers_and_retains_baseline() {
    for failure in 0..5 {
        let io = memory();
        let before = io.page.borrow().clone();
        if failure < 4 {
            io.fail_send.set(Some(failure));
        } else {
            io.fail_write.set(Some(0));
        }
        let mut baseline = None;
        assert!(transaction::apply(&io, &mut baseline, &request(&io)).is_err());
        assert_eq!(*io.page.borrow(), before);
        assert!(baseline.is_some());
    }
}
#[test]
fn failed_compensation_is_retained_and_explicit_restore_can_retry() {
    let io = memory();
    io.fail_all.set(true);
    let mut baseline = None;
    assert!(transaction::apply(&io, &mut baseline, &request(&io)).is_err());
    io.fail_all.set(false);
    let revision = capture(&io).unwrap().revision();
    transaction::restore(&io, &baseline, &revision).unwrap();
    assert_eq!(&io.page.borrow()[100..104], [3, 10, 20, 30]);
}
#[test]
fn exact_bit6_gate_takes_precedence_over_legacy_logo_layout() {
    let mut keyboard = [0; 256];
    keyboard[15] = 2;
    let mut flags = [0; 24];
    flags[0] = 1;
    flags[8] = 4;
    assert!(native::decode(&keyboard, &flags).unwrap().logo);
    flags[23] = 64;
    let caps = native::decode(&keyboard, &flags).unwrap();
    assert!(caps.top_cover && !caps.logo);
    flags[23] = 32;
    assert!(!native::decode(&keyboard, &flags).unwrap().top_cover);
    assert!(native::decode(&keyboard, &flags[..12]).is_err());
}
#[test]
fn unavailable_gate_and_stale_preview_produce_no_writes() {
    let io = memory();
    let target = request(&io);
    io.caps.borrow_mut().top_cover = false;
    assert!(transaction::apply(&io, &mut None, &target).is_err());
    assert!(io.words.borrow().is_empty());
}
