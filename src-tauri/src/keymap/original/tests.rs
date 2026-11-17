use super::*;

fn xml(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><root Active="0"><Profile_0 Name="原厂配置" IsSelected="1" IsKbEnable="1" IsMouseEnable="0">{body}</Profile_0></root>"#
    )
}
#[test]
fn original_composite_pause_and_old_hook_make_code_become_one_balanced_native_sequence() {
    let body = r#"<Set_0 Device="1" KeyCode="14753093" Action="1" IsKeypressDelay="1"><Index_0 KeyCode="57482" KeyMakeBreak="KeyMake" KeyDelay="0"/><Index_1 KeyCode="57629" KeyMakeBreak="KeyBreak" KeyDelay="7"/></Set_0>"#;
    let source = decode(xml(body).as_bytes()).unwrap();
    let binding = &source.configuration.devices[0].bindings[0];
    assert_eq!(binding.trigger, keymap::PAUSE_SCAN_CODE);
    let Action::Keys { steps } = &binding.action else {
        panic!()
    };
    assert!(steps.iter().all(|step| matches!(
        step,
        Step::Key {
            scan_code: keymap::PAUSE_SCAN_CODE,
            ..
        }
    )));
    keymap::validate_steps(steps).unwrap();
}
#[test]
fn original_keyboard_mouse_text_and_launch_all_decode_without_logging_contents() {
    let body = r#"<Set_1 Device="1" KeyCode="30" Action="1" IsKeypressDelay="1">
      <Index_0 KeyCode="57373" KeyMakeBreak="KeyMake" KeyDelay="0"/>
      <Index_1 KeyCode="57373" KeyMakeBreak="KeyBreak" KeyDelay="23"/></Set_1>
      <Set_2 Device="2" KeyCode="64" Action="4"/>
      <Set_3 Device="1" KeyCode="31" Action="3"><StartKey KeyCode="0" KeyMakeBreak="KeyMake" KeyDelay="0"/>
      <Text TextContent="中文&amp;🌀"/><SendKey KeyCode="0" KeyMakeBreak="KeyBreak" KeyDelay="0"/></Set_3>
      <Set_4 Device="1" KeyCode="32" Action="2"><Index_0 AppPath="C:\Apps\Editor.exe" AppEventId="256"/></Set_4>"#;
    let source = decode(xml(body).as_bytes()).unwrap();
    assert_eq!(source.name, "原厂配置");
    assert_eq!(source.configuration.devices[0].bindings.len(), 3);
    assert_eq!(source.configuration.devices[1].bindings[0].trigger, 64);
    assert_eq!(
        source.configuration.devices[0].bindings[0].action,
        Action::Keys {
            steps: vec![
                Step::Key {
                    scan_code: 57373,
                    pressed: true,
                    delay_after_ms: 23
                },
                Step::Key {
                    scan_code: 57373,
                    pressed: false,
                    delay_after_ms: 0
                }
            ]
        }
    );
    assert_eq!(
        source.configuration.devices[0].bindings[1].action,
        Action::Text {
            text: "中文&🌀".into(),
            before: vec![],
            after: vec![]
        }
    );
}
#[test]
fn delay_disabled_and_utf8_bom_keep_declared_semantics() {
    let body = r#"<Set_0 Device="1" KeyCode="30" Action="1" IsKeypressDelay="0"><Index_0 KeyCode="1" KeyMakeBreak="KeyMake" KeyDelay="8"/><Index_1 KeyCode="1" KeyMakeBreak="KeyBreak" KeyDelay="9"/></Set_0>"#;
    let source = decode(format!("\u{feff}{}", xml(body)).as_bytes()).unwrap();
    let Action::Keys { steps } = &source.configuration.devices[0].bindings[0].action else {
        panic!()
    };
    assert!(steps.iter().all(|step| matches!(
        step,
        Step::Key {
            delay_after_ms: 0,
            ..
        }
    )));
}
#[test]
fn missing_multiple_or_malformed_active_profiles_are_not_replaced_with_empty_defaults() {
    let root = xml("");
    assert!(
        decode(
            root.replace("IsSelected=\"1\"", "IsSelected=\"0\"")
                .as_bytes()
        )
        .is_err()
    );
    assert!(decode(root.replace("</root>",r#"<Profile_1 Name="second" IsSelected="1" IsKbEnable="0" IsMouseEnable="0"/></root>"#).as_bytes()).is_err());
    for body in [
        r#"<Set_1 Device="1" KeyCode="30" Action="99"/>"#,
        r#"<Unknown/>"#,
        r#"<Set_1 Device="1" KeyCode="30" Action="1" IsKeypressDelay="1"><Index_0 KeyCode="1" KeyMakeBreak="KeyMake" KeyDelay="0"/></Set_1>"#,
    ] {
        assert!(decode(xml(body).as_bytes()).is_err());
    }
    assert!(decode(b"<broken").is_err());
}
#[test]
fn input_fingerprint_includes_inactive_config_changes_and_file_reader_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.xml");
    assert!(read(&path).unwrap().is_none());
    let contents = xml("");
    std::fs::write(&path, contents.as_bytes()).unwrap();
    let first = read(&path).unwrap().unwrap();
    let changed = contents.replace(
        "</root>",
        r#"<Profile_1 IsSelected="0" Name="inactive"/></root>"#,
    );
    let second = decode(changed.as_bytes()).unwrap();
    assert_ne!(first.fingerprint, second.fingerprint);
    assert_eq!(first.configuration, second.configuration);
    assert_eq!(std::fs::read(&path).unwrap(), contents.as_bytes());
}
