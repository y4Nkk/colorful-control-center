use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn dtt_uses_original_integer_feature_branch_when_packet_has_no_embedded_flags() {
    let io = Memory::new();
    io.pages.borrow_mut()[7].fill(0);
    io.features.set([0, 1 << 12]);
    assert_eq!(capabilities(&io).unwrap(), [false, true]);
    io.pages.borrow_mut()[7][0] = 1;
    io.pages.borrow_mut()[7][16] = 16;
    assert_eq!(capabilities(&io).unwrap(), [true, true]);
    assert!(io.commands.borrow().is_empty());
}

struct Memory {
    features: Cell<[u32; 2]>,
    pages: RefCell<[[u8; 256]; 8]>,
    commands: RefCell<Vec<u32>>,
    fail_at: Cell<Option<(u8, usize)>>,
    fail_command: Cell<bool>,
}
impl Memory {
    fn new() -> Self {
        let mut pages = [[0; 256]; 8];
        pages[7][0] = 1;
        pages[7][11] = 16;
        pages[7][16] = 16;
        pages[7][17] = 15;
        pages[1][1] = 2;
        Self {
            features: Cell::new([0, 0]),
            pages: RefCell::new(pages),
            commands: Default::default(),
            fail_at: Cell::new(None),
            fail_command: Cell::new(false),
        }
    }
    fn request(&self, turbo: Option<bool>, dtt: Option<bool>) -> Request {
        Request {
            expected_revision: preview(self, Some(true)).unwrap().revision,
            quiet_fan: None,
            replace_custom_fan: false,
            values: Values {
                turbo_fan: turbo,
                dtt,
            },
        }
    }
}
impl SettingsAccess for Memory {
    fn read(&self, page: u8, offset: usize, count: usize) -> Result<Vec<u8>, String> {
        Ok(self.pages.borrow()[page as usize][offset..offset + count].to_vec())
    }
    fn write(&self, page: u8, offset: usize, data: &[u8]) -> Result<(), String> {
        if self.fail_at.get() == Some((page, offset)) {
            self.fail_at.set(None);
            return Err("隔离选项保存失败".into());
        }
        self.pages.borrow_mut()[page as usize][offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }
}
impl ModeIo for Memory {
    fn feature_flags(&self, command: i32) -> Result<u32> {
        Ok(self.features.get()[usize::from(command == 96)])
    }
    fn fan_info(&self) -> Result<[u8; 256]> {
        Ok([0; 256])
    }
    fn table(&self, _: u8) -> Result<[u8; 256]> {
        Ok([0; 256])
    }
    fn send_mode(&self, command: u32) -> Result<()> {
        self.commands.borrow_mut().push(command);
        if self.fail_command.replace(false) {
            bail!("隔离命令失败");
        }
        Ok(())
    }
}
#[test]
fn independent_flag_combinations_pack_source_bits_without_cpu_boost_alias() {
    for (turbo, dtt, command) in [
        (false, false, 2),
        (true, false, 0x42),
        (false, true, 0x82),
        (true, true, 0xC2),
    ] {
        let io = Memory::new();
        let p = preview(&io, Some(true)).unwrap();
        let (target, _) = p.target(&io.request(Some(turbo), Some(dtt))).unwrap();
        write_target(&io, target).unwrap();
        assert_eq!(*io.commands.borrow(), [command]);
        assert_eq!(
            capture(&io, false).unwrap().preferences,
            [u8::from(turbo), u8::from(dtt)]
        );
    }
}
#[test]
fn dtt_uses_wmi96_and_does_not_alias_the_wmi122_light_bar_bit() {
    let io = Memory::new();
    io.pages.borrow_mut()[7][11] = 0;
    io.pages.borrow_mut()[7][7] = 16;
    assert_eq!(capabilities(&io).unwrap(), [true, false]);
    let request = io.request(Some(false), Some(true));
    assert!(preview(&io, Some(true)).unwrap().target(&request).is_err());
    assert!(io.commands.borrow().is_empty());
    io.pages.borrow_mut()[7][7] = 0;
    io.pages.borrow_mut()[7][11] = 16;
    assert_eq!(capabilities(&io).unwrap(), [true, true]);
}
#[test]
fn capabilities_are_versioned_and_unsupported_fields_cannot_be_written() {
    let io = Memory::new();
    io.pages.borrow_mut()[7][16] = 0;
    let p = preview(&io, Some(true)).unwrap();
    assert_eq!(p.values().turbo_fan, None);
    assert!(p.target(&io.request(Some(true), Some(false))).is_err());
    assert!(p.target(&io.request(None, Some(true))).is_ok());
    io.pages.borrow_mut()[7][0] = 0;
    assert_eq!(capabilities(&io).unwrap(), [false; 2]);
    io.pages.borrow_mut()[7][0] = 2;
    assert!(capabilities(&io).is_err());
    assert!(io.commands.borrow().is_empty());
}
#[test]
fn other_modes_and_unknown_preference_refuse() {
    let io = Memory::new();
    for mode in [0, 1, 4] {
        io.pages.borrow_mut()[1][1] = mode;
        assert!(
            preview(&io, Some(true))
                .unwrap()
                .target(&io.request(Some(true), Some(true)))
                .is_err()
        );
    }
    io.pages.borrow_mut()[4][8] = 2;
    assert!(preview(&io, Some(true)).is_err());
    assert!(capture(&io, false).is_err());
}
#[test]
fn noiseless_is_bound_to_capability_mode_and_remembered_preference() {
    let io = Memory::new();
    let mut request = io.request(Some(false), Some(false));
    request.quiet_fan = Some(true);
    assert!(preview(&io, Some(true)).unwrap().target(&request).is_err());
    io.pages.borrow_mut()[7][20] = 1;
    let p = preview(&io, Some(true)).unwrap();
    assert!(!p.quiet_fan.as_ref().unwrap().remembered);
    request.expected_revision = p.revision.clone();
    assert_eq!(p.target(&request).unwrap().1, Some(FAN_MODE_NOISELESS));
    request.values.turbo_fan = Some(true);
    assert!(p.target(&request).is_err());

    io.pages.borrow_mut()[7][21] = 1;
    io.pages.borrow_mut()[4][72] = FAN_MODE_NOISELESS;
    let saved = preview(&io, Some(true)).unwrap();
    let quiet = saved.quiet_fan.as_ref().unwrap();
    assert!(quiet.enabled && quiet.remembered && !quiet.active);
    assert_ne!(saved.revision, p.revision);

    io.pages.borrow_mut()[1][1] = 3;
    let entertainment = preview(&io, Some(true)).unwrap();
    let quiet = entertainment.quiet_fan.as_ref().unwrap();
    assert!(!quiet.enabled && quiet.remembered && !quiet.active);
    assert!(entertainment.target(&request).is_err());
    request.expected_revision = entertainment.revision.clone();
    assert_eq!(
        entertainment.target(&request).unwrap().1,
        Some(FAN_MODE_NOISELESS)
    );
    io.pages.borrow_mut()[4][9] = 1;
    assert!(preview(&io, Some(true)).unwrap().target(&request).is_err());
    io.pages.borrow_mut()[1][1] = 0;
    assert!(preview(&io, Some(true)).unwrap().quiet_fan.is_none());
}
#[test]
fn missing_or_unknown_driver_blocks_new_dtt_but_allows_switching_it_off() {
    for driver in [None, Some(false)] {
        let io = Memory::new();
        let p = preview(&io, driver).unwrap();
        let mut request = io.request(Some(false), Some(true));
        request.expected_revision = p.revision.clone();
        assert!(p.target(&request).is_err());
        io.pages.borrow_mut()[1][32] = 1;
        let p = preview(&io, driver).unwrap();
        request.values.dtt = Some(false);
        request.expected_revision = p.revision.clone();
        assert!(p.target(&request).is_ok());
    }
}
#[test]
fn custom_fan_blocks_turbo_change_without_blocking_independent_dtt() {
    let io = Memory::new();
    io.pages.borrow_mut()[4][5] = 6;
    let p = preview(&io, Some(true)).unwrap();
    assert!(p.target(&io.request(Some(true), Some(false))).is_err());
    assert!(p.target(&io.request(Some(false), Some(true))).is_ok());
}
#[test]
fn revision_binds_mode_flags_caps_fan_and_driver_presence() {
    for changed in 0..5 {
        let io = Memory::new();
        let request = io.request(Some(true), Some(true));
        let driver = match changed {
            0 => {
                io.pages.borrow_mut()[1][1] = 3;
                Some(true)
            }
            1 => {
                io.pages.borrow_mut()[4][8] = 1;
                Some(true)
            }
            2 => {
                io.pages.borrow_mut()[7][16] = 0;
                Some(true)
            }
            3 => {
                io.pages.borrow_mut()[4][5] = 1;
                Some(true)
            }
            _ => Some(false),
        };
        assert!(preview(&io, driver).unwrap().target(&request).is_err());
        assert!(io.commands.borrow().is_empty());
    }
}
#[test]
fn shared_mode_snapshot_restores_preferences_after_a_later_mode_change() {
    let io = Memory::new();
    let before = capture(&io, false).unwrap();
    let (next, _) = preview(&io, Some(true))
        .unwrap()
        .target(&io.request(Some(true), Some(true)))
        .unwrap();
    write_target(&io, next).unwrap();
    io.pages.borrow_mut()[1][1] = 3;
    recover(&io, before).unwrap();
    assert_eq!(capture(&io, false).unwrap().preferences, [0, 0]);
    assert_eq!(byte(&io, 1, 1).unwrap(), 2);
    assert_eq!(io.commands.borrow().last(), Some(&2));
}
#[test]
fn save_and_command_failures_can_restore_both_original_flags() {
    for failure in 0..3 {
        let io = Memory::new();
        let before = capture(&io, false).unwrap();
        let p = preview(&io, Some(true)).unwrap();
        let (next, _) = p.target(&io.request(Some(true), Some(true))).unwrap();
        match failure {
            0 => io.fail_at.set(Some((4, 8))),
            1 => io.fail_at.set(Some((1, 32))),
            _ => io.fail_command.set(true),
        }
        assert!(write_target(&io, next).is_err());
        recover(&io, before).unwrap();
        assert_eq!(capture(&io, false).unwrap().preferences, [0, 0]);
        assert_eq!(io.commands.borrow().last(), Some(&2));
    }
}
#[test]
fn recovery_continues_other_preferences_and_mode_when_one_save_fails() {
    let io = Memory::new();
    let before = capture(&io, false).unwrap();
    io.pages.borrow_mut()[4][8] = 1;
    io.pages.borrow_mut()[1][32] = 1;
    io.pages.borrow_mut()[1][1] = 3;
    io.fail_at.set(Some((4, 8)));
    assert!(recover(&io, before).is_err());
    assert_eq!(byte(&io, 1, 32).unwrap(), 0);
    assert_eq!(byte(&io, 1, 1).unwrap(), 2);
    assert_eq!(io.commands.borrow().last(), Some(&2));
    recover(&io, before).unwrap();
    assert_eq!(capture(&io, false).unwrap().preferences, [0, 0]);
}
#[test]
fn target_contract_requires_curve_confirmation_flag_and_rejects_extra_fields() {
    assert!(
        serde_json::from_str::<Request>(
            r#"{"expectedRevision":"x","quietFan":null,"replaceCustomFan":false,"values":{"turboFan":true,"dtt":true}}"#
        )
        .is_ok()
    );
    assert!(
        serde_json::from_str::<Request>(
            r#"{"expectedRevision":"x","quietFan":null,"values":{"turboFan":true,"dtt":true}}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<Request>(
            r#"{"expectedRevision":"x","quietFan":null,"replaceCustomFan":false,"values":{"turboFan":true,"dtt":true,"cpuBoost":true}}"#
        )
        .is_err()
    );
}
