use super::{
    Decision::*, Facts, Firmware, Trigger::*, candidates, decide, lowest, original_saved_byte,
    saved_or_highest,
};

fn facts() -> Facts {
    Facts {
        customer: 1,
        mode: 1,
        previous_mode: Option::None,
        on_ac: Some(false),
        supported: true,
        checked: true,
        trigger: SupplyChange,
    }
}

#[test]
fn firmware_flags_follow_the_original_versioned_defaults() {
    let mut packet = [0; 256];
    packet[18] = 0xff;
    assert_eq!(
        Firmware::from_packet(&packet).unwrap(),
        Firmware {
            supported: true,
            default_enabled: true
        }
    );
    packet[0] = 1;
    for (value, supported, default_enabled) in [
        (0, true, true),
        (0x20, false, true),
        (0x80, true, false),
        (0xa0, false, false),
    ] {
        packet[18] = value;
        assert_eq!(
            Firmware::from_packet(&packet).unwrap(),
            Firmware {
                supported,
                default_enabled
            }
        );
    }
    packet[0] = 2;
    assert!(Firmware::from_packet(&packet).is_err());
}

#[test]
fn ordinary_customer_uses_mode_previous_mode_and_power_source() {
    let mut item = facts();
    assert_eq!(decide(item), Ok(SaveThenLow));
    item.on_ac = Some(true);
    assert_eq!(decide(item), Ok(RestoreSaved));
    item.previous_mode = Some(2);
    assert_eq!(decide(item), Ok(None));
    item.trigger = DcModeChange;
    assert_eq!(decide(item), Ok(RestoreSaved));
    item.mode = 2;
    item.previous_mode = Some(1);
    assert_eq!(decide(item), Ok(RestoreSaved));
    item.trigger = ModeChange;
    assert_eq!(decide(item), Ok(None));
    item.on_ac = Some(false);
    assert_eq!(decide(item), Ok(RestoreSaved));
    item.previous_mode = Some(3);
    assert_eq!(decide(item), Ok(None));
}

#[test]
fn disabled_policy_restore_and_unknown_supply_are_not_guessed() {
    let mut item = facts();
    item.checked = false;
    assert_eq!(decide(item), Ok(RestoreSaved));
    item.on_ac = Some(true);
    assert_eq!(decide(item), Ok(None));
    item.trigger = DcModeChange;
    item.on_ac = Option::None;
    assert_eq!(decide(item), Ok(RestoreSaved));
    item.trigger = SupplyChange;
    assert!(decide(item).is_err());
    item.supported = false;
    assert_eq!(decide(item), Ok(None));
    item.supported = true;
    item.trigger = Resume;
    assert_eq!(decide(item), Ok(None));
    item.trigger = Startup;
    item.mode = 5;
    assert!(decide(item).is_err());
}

#[test]
fn customer_five_has_its_own_original_mode_and_trigger_table() {
    let mut item = facts();
    item.customer = 5;
    item.on_ac = Option::None;
    assert_eq!(decide(item), Ok(None));
    item.trigger = Startup;
    assert_eq!(decide(item), Ok(Low));
    item.mode = 0;
    item.trigger = ModeChange;
    item.previous_mode = Some(2);
    assert_eq!(decide(item), Ok(SaveThenLow));
    item.previous_mode = Some(1);
    assert_eq!(decide(item), Ok(Low));
    item.mode = 3;
    assert_eq!(decide(item), Ok(RestoreSaved));
    item.previous_mode = Some(2);
    assert_eq!(decide(item), Ok(None));
    item.mode = 4;
    assert!(decide(item).is_err());
    item.checked = false;
    item.mode = 0;
    assert_eq!(decide(item), Ok(RestoreSaved));
    item.trigger = Resume;
    assert_eq!(decide(item), Ok(None));
}

#[test]
fn candidate_selection_and_original_byte_truncation_are_explicit() {
    let frequencies = [144, 48, 60, 30, 144, 40, 240];
    assert_eq!(candidates(&frequencies), [40, 60, 144, 240]);
    assert_eq!(lowest(&frequencies), Ok(40));
    assert_eq!(saved_or_highest(&frequencies, 144), Ok(144));
    assert_eq!(saved_or_highest(&frequencies, 0), Ok(240));
    assert_eq!(original_saved_byte(384), 128);
    assert!(lowest(&[30, 48]).is_err());
    assert!(saved_or_highest(&[], 60).is_err());
}
