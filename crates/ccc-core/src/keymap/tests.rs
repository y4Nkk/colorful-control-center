use super::*;
use compile::Item;

#[test]
fn original_pause_values_normalize_to_the_driver_wire_code_without_truncating_unknown_codes() {
    for raw in [0xe11d, 0xe11d45, 0xe08a] {
        assert_eq!(original_scan_code(raw).unwrap(), PAUSE_SCAN_CODE);
    }
    assert_eq!(original_scan_code(69).unwrap(), 69);
    assert!(original_scan_code(0x123456).is_err());
    let mut cfg = config(Action::Keys {
        steps: vec![
            Step::Key {
                scan_code: PAUSE_SCAN_CODE,
                pressed: true,
                delay_after_ms: 3,
            },
            Step::Key {
                scan_code: PAUSE_SCAN_CODE,
                pressed: false,
                delay_after_ms: 0,
            },
        ],
    });
    cfg.devices[0].bindings[0].trigger = PAUSE_SCAN_CODE;
    let compiled = compile::configuration(&cfg).unwrap();
    assert!(matches!(
        compiled[0].bindings[0].actions[0],
        Item::Down {
            code: PAUSE_SCAN_CODE,
            ..
        }
    ));
}

fn pair() -> Vec<Step> {
    vec![
        Step::Key {
            scan_code: 0xe01d,
            pressed: true,
            delay_after_ms: 17,
        },
        Step::Key {
            scan_code: 0xe01d,
            pressed: false,
            delay_after_ms: 29,
        },
    ]
}
fn config(action: Action) -> Configuration {
    Configuration {
        devices: vec![DeviceSettings {
            device: Device::Keyboard,
            enabled: true,
            bindings: vec![Binding {
                trigger: 30,
                action,
            }],
        }],
    }
}
#[test]
fn normal_and_both_oem_fn_prefixes_use_scan_codes_without_vk_translation() {
    let plan = compile::configuration(&config(Action::Keys { steps: pair() })).unwrap();
    let binding = &plan[0].bindings[0];
    let hotkey = |code| Item::Click {
        kind: 0,
        code,
        make_delay_ms: 0,
        break_delay_ms: 0,
    };
    assert_eq!(
        binding.hotkeys,
        vec![
            vec![hotkey(30)],
            vec![hotkey(0xe077), hotkey(30)],
            vec![hotkey(0xe078), hotkey(30)]
        ]
    );
    assert_eq!(
        binding.actions,
        vec![
            Item::Down {
                code: 0xe01d,
                delay_ms: 17
            },
            Item::Up {
                code: 0xe01d,
                delay_ms: 29
            }
        ]
    );
}
#[test]
fn five_mouse_triggers_have_one_alias_and_mouse_actions_keep_the_native_key_type() {
    for button in [1, 4, 16, 64, 256] {
        let cfg = Configuration {
            devices: vec![DeviceSettings {
                device: Device::Mouse,
                enabled: false,
                bindings: vec![Binding {
                    trigger: button,
                    action: Action::Keys {
                        steps: vec![Step::MouseClick {
                            button,
                            make_delay_ms: 3,
                            break_delay_ms: 4,
                        }],
                    },
                }],
            }],
        };
        let plan = compile::configuration(&cfg).unwrap();
        assert!(!plan[0].enabled);
        assert_eq!(plan[0].bindings[0].hotkeys.len(), 1);
        assert_eq!(
            plan[0].bindings[0].actions,
            vec![Item::Click {
                kind: 1,
                code: button,
                make_delay_ms: 3,
                break_delay_ms: 4
            }]
        );
    }
    for button in [0, 2, 3, 255] {
        assert!(validate_code(Device::Mouse, button).is_err());
    }
}
#[test]
fn text_preserves_surrogate_pairs_null_termination_and_before_after_order() {
    let click = Click {
        scan_code: 0x1d,
        make_delay_ms: 1,
        break_delay_ms: 2,
    };
    let plan = compile::configuration(&config(Action::Text {
        text: "中文🌀".into(),
        before: vec![click.clone()],
        after: vec![click],
    }))
    .unwrap();
    assert_eq!(
        plan[0].bindings[0].actions,
        vec![
            Item::Click {
                kind: 0,
                code: 0x1d,
                make_delay_ms: 1,
                break_delay_ms: 2
            },
            Item::Text(vec![0x4e2d, 0x6587, 0xd83c, 0xdf00, 0]),
            Item::Click {
                kind: 0,
                code: 0x1d,
                make_delay_ms: 1,
                break_delay_ms: 2
            },
        ]
    );
    for text in ["", "a\0b"] {
        assert!(
            validate(&config(Action::Text {
                text: text.into(),
                before: vec![],
                after: vec![]
            }))
            .is_err()
        );
    }
}
#[test]
fn disabled_binding_is_an_empty_action_collection_and_program_events_use_type_three() {
    assert!(
        compile::configuration(&config(Action::Disable)).unwrap()[0].bindings[0]
            .actions
            .is_empty()
    );
    let plan = compile::configuration(&config(Action::Launch {
        path: r"C:\Apps\Editor.exe".into(),
        event_id: 256,
    }))
    .unwrap();
    assert_eq!(
        plan[0].bindings[0].actions,
        vec![Item::Click {
            kind: 3,
            code: 256,
            make_delay_ms: 0,
            break_delay_ms: 0
        }]
    );
    assert!(
        validate(&config(Action::Launch {
            path: "".into(),
            event_id: 256
        }))
        .is_err()
    );
    assert!(
        validate(&config(Action::Launch {
            path: "x.exe".into(),
            event_id: 255
        }))
        .is_err()
    );
}
#[test]
fn exact_native_sequence_capacity_is_accepted_without_truncation_and_held_keys_are_rejected() {
    let steps: Vec<_> = (0..128).flat_map(|_| pair()).collect();
    assert_eq!(
        compile::configuration(&config(Action::Keys {
            steps: steps.clone()
        }))
        .unwrap()[0]
            .bindings[0]
            .actions
            .len(),
        256
    );
    let mut too_many = steps;
    too_many.extend(pair());
    assert!(validate_steps(&too_many).is_err());
    assert!(validate_steps(&[]).is_err());
    assert!(validate_steps(&pair()[..1]).is_err());
    assert!(validate_steps(&pair()[1..]).is_err());
    let steps = vec![pair()[0].clone(), pair()[0].clone(), pair()[1].clone()];
    validate_steps(&steps).unwrap();
}
#[test]
fn duplicate_devices_triggers_event_ids_and_reserved_prefixes_are_not_silently_merged() {
    let mut cfg = config(Action::Disable);
    cfg.devices.push(cfg.devices[0].clone());
    assert!(validate(&cfg).is_err());
    let mut cfg = config(Action::Disable);
    let duplicate = cfg.devices[0].bindings[0].clone();
    cfg.devices[0].bindings.push(duplicate);
    assert!(validate(&cfg).is_err());
    for code in [0, 0xe077, 0xe078] {
        let mut cfg = config(Action::Disable);
        cfg.devices[0].bindings[0].trigger = code;
        assert!(validate(&cfg).is_err());
    }
    let mut cfg = config(Action::Launch {
        path: "a.exe".into(),
        event_id: 256,
    });
    cfg.devices[0].bindings.push(Binding {
        trigger: 31,
        action: Action::Launch {
            path: "b.exe".into(),
            event_id: 256,
        },
    });
    assert!(validate(&cfg).is_err());
}
#[test]
fn delay_values_keep_the_full_ulong_precision() {
    let cfg = config(Action::Keys {
        steps: vec![
            Step::Key {
                scan_code: 1,
                pressed: true,
                delay_after_ms: u32::MAX,
            },
            Step::Key {
                scan_code: 1,
                pressed: false,
                delay_after_ms: 0,
            },
        ],
    });
    assert_eq!(
        compile::configuration(&cfg).unwrap()[0].bindings[0].actions[0],
        Item::Down {
            code: 1,
            delay_ms: u32::MAX
        }
    );
}
