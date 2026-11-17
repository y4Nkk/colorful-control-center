use super::*;
use std::cell::{Cell, RefCell};

struct Memory {
    page: RefCell<Vec<u8>>,
    caps: RefCell<Capabilities>,
    words: RefCell<Vec<(i32, u32)>>,
    writes: Cell<usize>,
    fail_send: Cell<Option<usize>>,
    fail_write: Cell<Option<usize>>,
    fail_all: Cell<bool>,
}
#[path = "cover_tests.rs"]
mod cover_tests;
impl Memory {
    fn new() -> Self {
        let mut page = vec![0; 256];
        page[35] = 3;
        page[36..40].copy_from_slice(&[1, 0, 0, 30]);
        page[80] = 1;
        page[84] = 1;
        page[92] = 1;
        page[96..100].copy_from_slice(&[1, 0, 0, 255]);
        page[100..104].copy_from_slice(&[1, 255, 0, 0]);
        page[104..108].copy_from_slice(&[0, 0, 0, 255]);
        Self {
            page: RefCell::new(page),
            caps: RefCell::new(Capabilities {
                keyboard_type: 2,
                peripheral_support_known: true,
                boot_override: true,
                logo: true,
                light_bar: true,
                top_cover: false,
            }),
            words: Default::default(),
            writes: Cell::new(0),
            fail_send: Cell::new(None),
            fail_write: Cell::new(None),
            fail_all: Cell::new(false),
        }
    }
    fn request(&self) -> Request {
        let before = capture(self).unwrap();
        let mut values = before.values().unwrap();
        values.idle_timer.seconds = 7199;
        values.boot_override = Some(true);
        values.logo = Some(Zone {
            enabled: false,
            color: Rgb { r: 0, g: 255, b: 0 },
        });
        values.light_bar = Some(Zone {
            enabled: true,
            color: Rgb { r: 0, g: 0, b: 127 },
        });
        Request {
            expected_revision: before.revision(),
            values,
        }
    }
}
impl SettingsAccess for Memory {
    fn read(&self, page: u8, offset: usize, length: usize) -> Result<Vec<u8>, String> {
        assert_eq!(page, 2);
        Ok(self.page.borrow()[offset..offset + length].to_vec())
    }
    fn write(&self, page: u8, offset: usize, bytes: &[u8]) -> Result<(), String> {
        assert_eq!(page, 2);
        let index = self.writes.get();
        self.writes.set(index + 1);
        self.page.borrow_mut()[offset..offset + bytes.len()].copy_from_slice(bytes);
        if self.fail_write.get() == Some(index) {
            self.fail_write.set(None);
            return Err("隔离部分保存失败".into());
        }
        Ok(())
    }
}
impl Io for Memory {
    fn capabilities(&self) -> Result<Capabilities> {
        Ok(self.caps.borrow().clone())
    }
    fn send(&self, command: i32, word: u32) -> Result<()> {
        let index = self.words.borrow().len();
        self.words.borrow_mut().push((command, word));
        if self.fail_all.get() || self.fail_send.get() == Some(index) {
            self.fail_send.set(None);
            bail!("隔离灯光命令失败");
        }
        Ok(())
    }
}
#[test]
fn exact_timer_boot_colors_and_light_bar_mask_match_source() {
    let io = Memory::new();
    let request = io.request();
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &request).unwrap();
    assert_eq!(
        *io.words.borrow(),
        [
            (121, 0x181C1FFF),
            (121, 0x1E000001),
            (103, 0xF60000FF),
            (103, 0xF3640000),
            (103, 0xE0059001)
        ]
    );
    assert_eq!(&io.page.borrow()[36..40], [1, 1, 59, 59]);
    assert_eq!(capture(&io).unwrap().values().unwrap(), request.values);
    assert!(baseline.is_some());
}
#[test]
fn disabled_initial_timer_zero_and_boot_false_are_exactly_restored() {
    let io = Memory::new();
    io.page.borrow_mut()[36..40].fill(0);
    let original = io.page.borrow().clone();
    let mut request = io.request();
    request.values.idle_timer.enabled = true;
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &request).unwrap();
    io.words.borrow_mut().clear();
    transaction::restore(&io, &baseline, &capture(&io).unwrap().revision()).unwrap();
    assert_eq!(io.page.borrow().as_slice(), original);
    assert_eq!(io.words.borrow()[0], (121, 0x18000000));
    assert_eq!(io.words.borrow()[1], (121, 0x1E000000));
}
#[test]
fn each_partial_command_and_save_failure_restores_all_owned_fields() {
    for (command_failure, count) in [(true, 5), (false, 4)] {
        for n in 0..count {
            let io = Memory::new();
            let original = io.page.borrow().clone();
            if command_failure {
                io.fail_send.set(Some(n));
            } else {
                io.fail_write.set(Some(n));
            }
            let mut baseline = None;
            assert!(transaction::apply(&io, &mut baseline, &io.request()).is_err());
            assert_eq!(*io.page.borrow(), original);
            assert!(baseline.is_some());
            assert!(
                io.words
                    .borrow()
                    .iter()
                    .any(|word| *word == (121, 0x18001EFF))
            );
            assert_eq!(*io.words.borrow().last().unwrap(), (103, 0xE0053001));
        }
    }
}
#[test]
fn recovery_continues_after_raw_failure_and_retry_preserves_other_owners() {
    let io = Memory::new();
    let original = io.page.borrow().clone();
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &io.request()).unwrap();
    for (at, value) in [(32, 6), (33, 1), (35, 4), (84, 0), (97, 9), (128, 87)] {
        io.page.borrow_mut()[at] = value;
    }
    io.fail_all.set(true);
    io.words.borrow_mut().clear();
    let error = transaction::restore(&io, &baseline, &capture(&io).unwrap().revision())
        .unwrap_err()
        .to_string();
    assert!(error.contains("灯光命令恢复"));
    assert_eq!(io.words.borrow().len(), 5);
    assert_eq!(&io.page.borrow()[100..108], &original[100..108]);
    for (at, value) in [(32, 6), (33, 1), (35, 4), (84, 0), (97, 9), (128, 87)] {
        assert_eq!(io.page.borrow()[at], value);
    }
    io.fail_all.set(false);
    transaction::restore(&io, &baseline, &capture(&io).unwrap().revision()).unwrap();
}
#[test]
fn preview_binds_current_shared_mask_and_capabilities_before_writes() {
    for mutate in 0..3 {
        let io = Memory::new();
        let request = io.request();
        match mutate {
            0 => io.page.borrow_mut()[84] = 0,
            1 => io.page.borrow_mut()[100] = 0,
            _ => io.caps.borrow_mut().keyboard_type = 6,
        }
        assert!(transaction::apply(&io, &mut None, &request).is_err());
        assert!(io.words.borrow().is_empty());
        assert_eq!(io.writes.get(), 0);
    }
}
#[test]
fn invalid_and_unsupported_targets_or_unknown_shared_flags_never_write() {
    for case in 0..5 {
        let io = Memory::new();
        let mut request = io.request();
        match case {
            0 => request.values.idle_timer.seconds = 4,
            1 => request.values.idle_timer.seconds = 7200,
            2 => request.values.logo = None,
            3 => request.values.boot_override = None,
            _ => {
                io.page.borrow_mut()[84] = 2;
                request.expected_revision = capture(&io).unwrap().revision();
            }
        }
        assert!(transaction::apply(&io, &mut None, &request).is_err());
        assert!(io.words.borrow().is_empty());
        assert_eq!(io.writes.get(), 0);
    }
}
#[test]
fn off_keyboard_is_not_turned_on_by_peripheral_changes() {
    let io = Memory::new();
    io.page.borrow_mut()[80] = 0;
    transaction::apply(&io, &mut None, &io.request()).unwrap();
    assert_eq!(*io.words.borrow().last().unwrap(), (103, 0xE0000007));
    assert_eq!(io.page.borrow()[35], 3);
    assert_eq!(io.page.borrow()[80], 0);
}
#[test]
fn current_contract_rejects_legacy_and_unknown_fields() {
    let io = Memory::new();
    let request = io.request();
    let mut json =
        serde_json::json!({"expectedRevision":request.expected_revision,"values":request.values});
    assert!(serde_json::from_value::<Request>(json.clone()).is_ok());
    json["values"]["tpColor"] = serde_json::json!({"r":1,"g":2,"b":3});
    assert!(serde_json::from_value::<Request>(json).is_err());
}
#[test]
fn unchanged_values_do_not_create_baseline_or_send_commands() {
    let io = Memory::new();
    let before = capture(&io).unwrap();
    let request = Request {
        expected_revision: before.revision(),
        values: before.values().unwrap(),
    };
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &request).unwrap();
    assert!(baseline.is_none());
    assert!(io.words.borrow().is_empty());
}
#[test]
fn capability_words_for_light_bar_and_dtt_are_distinct_and_versioned() {
    let mut keyboard = [0; 256];
    keyboard[15] = 2;
    keyboard[43] = 1;
    let mut flags = [0; 24];
    flags[0] = 1;
    flags[11] = 16;
    let caps = native::decode(&keyboard, &flags).unwrap();
    assert!(caps.boot_override);
    assert!(!caps.light_bar);
    assert!(!caps.logo);
    flags[7] = 16;
    flags[8] = 4;
    let caps = native::decode(&keyboard, &flags).unwrap();
    assert!(caps.light_bar && caps.logo);
    keyboard[15] = 3;
    let caps = native::decode(&keyboard, &flags).unwrap();
    assert!(!caps.light_bar && !caps.logo && caps.boot_override);
    flags[0] = 0;
    assert!(
        !native::decode(&keyboard, &flags)
            .unwrap()
            .peripheral_support_known
    );
    flags[0] = 2;
    assert!(native::decode(&keyboard, &flags).is_err());
    assert!(native::decode(&keyboard, &flags[..11]).is_err());
}
#[test]
fn initial_restore_rejects_old_preview_or_changed_device_without_writes() {
    let io = Memory::new();
    let request = io.request();
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &request).unwrap();
    io.words.borrow_mut().clear();
    assert!(transaction::restore(&io, &baseline, &request.expected_revision).is_err());
    let revision = capture(&io).unwrap().revision();
    io.caps.borrow_mut().keyboard_type = 6;
    assert!(transaction::restore(&io, &baseline, &revision).is_err());
    assert!(io.words.borrow().is_empty());
}
