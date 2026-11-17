use super::*;
#[path = "type38_tests.rs"]
mod type38;
use anyhow::bail;
use std::cell::Cell;

pub(super) struct MockIo {
    pub(super) white_report: Cell<Option<i32>>,
    pub(super) change_cap_at: Cell<Option<usize>>,
    pub(super) words: Mutex<Vec<(i32, u32)>>,
    pub(super) page: Mutex<[u8; 256]>,
    pub(super) fail_at: Cell<Option<usize>>,
    pub(super) fail_saved_at: Cell<Option<usize>>,
    pub(super) ignore_saved_at: Cell<Option<usize>>,
    pub(super) saved_writes: Cell<usize>,
    pub(super) fail_read: Cell<bool>,
    pub(super) fail_all_raw: Cell<bool>,
    pub(super) light_bar: Cell<bool>,
    pub(super) cycle_logo: Cell<bool>,
    pub(super) cycle_manufacturer: std::cell::RefCell<String>,
    pub(super) cycle_customer: Cell<u8>,
}
impl settings::SettingsAccess for MockIo {
    fn read(&self, _: u8, offset: usize, length: usize) -> std::result::Result<Vec<u8>, String> {
        Ok(self.page.lock().unwrap()[offset..offset + length].to_vec())
    }
    fn write(&self, _: u8, offset: usize, data: &[u8]) -> std::result::Result<(), String> {
        let n = self.saved_writes.get();
        self.saved_writes.set(n + 1);
        if self.ignore_saved_at.get() == Some(n) {
            return Ok(());
        }
        self.page.lock().unwrap()[offset..offset + data.len()].copy_from_slice(data);
        if self.fail_saved_at.get() == Some(n) {
            self.fail_saved_at.set(None);
            return Err("模拟部分保存后失败".into());
        }
        Ok(())
    }
}
impl LightingIo for MockIo {
    fn white_brightness(&self) -> Result<i32> {
        self.white_report
            .get()
            .ok_or_else(|| anyhow!("模拟白光报告读取失败"))
    }
    fn cycle_info(&self, raw_type: u8) -> Result<color_cycle::Info> {
        Ok(color_cycle::Info {
            keyboard_type: raw_type,
            customer_id: self.cycle_customer.get(),
            monster_manufacturer: self.cycle_manufacturer.borrow().contains("MONSTER"),
            logo: self.cycle_logo.get(),
            light_bar: self.light_bar.get(),
        })
    }
    fn cycle_index(&self) -> Result<u8> {
        Ok(self.page.lock().unwrap()[127])
    }
    fn light_bar(&self, _: Option<u8>) -> Result<bool> {
        Ok(self.light_bar.get())
    }
    fn write_word(&self, command: i32, value: u32) -> Result<()> {
        if self.fail_all_raw.get() {
            bail!("模拟立即通道持续故障");
        }
        let mut words = self.words.lock().unwrap();
        if self.change_cap_at.get() == Some(words.len()) {
            self.change_cap_at.set(None);
            self.light_bar.set(!self.light_bar.get());
        }
        if self.fail_at.get() == Some(words.len()) {
            self.fail_at.set(None);
            bail!("模拟立即写失败");
        }
        words.push((command, value));
        Ok(())
    }
    fn read_saved_block(&self) -> Result<SavedBlock> {
        if self.fail_read.get() {
            bail!("模拟保存块读取失败");
        }
        SavedBlock::from_bytes(self.page.lock().unwrap()[32..108].to_vec())
    }
    fn apply_saved(&self, patches: &[Patch]) -> Result<()> {
        settings::apply(self, PAGE, patches, None)
            .map(|_| ())
            .map_err(|e| anyhow!(e))
    }
}
pub(super) fn io(page: [u8; 256]) -> MockIo {
    MockIo {
        white_report: Cell::new(Some(2)),
        change_cap_at: Cell::new(None),
        words: Mutex::new(vec![]),
        page: Mutex::new(page),
        fail_at: Cell::new(None),
        fail_saved_at: Cell::new(None),
        ignore_saved_at: Cell::new(None),
        saved_writes: Cell::new(0),
        fail_read: Cell::new(false),
        fail_all_raw: Cell::new(false),
        light_bar: Cell::new(false),
        cycle_logo: Cell::new(false),
        cycle_manufacturer: std::cell::RefCell::new("Notebook".into()),
        cycle_customer: Cell::new(1),
    }
}
pub(super) fn preset() -> [u8; 256] {
    let mut page = [0; 256];
    for (at, bytes) in [
        (32, vec![4]),
        (35, vec![4]),
        (80, vec![1]),
        (81, vec![10, 20, 30]),
        (84, vec![1, 11, 22, 33]),
        (88, vec![0, 44, 55, 66]),
        (92, vec![1, 77, 88, 99]),
        (96, vec![1, 0, 0, 255]),
        (100, vec![1, 4, 5, 6]),
    ] {
        page[at..at + bytes.len()].copy_from_slice(&bytes);
    }
    page
}
pub(super) fn rgb(effect: Effect) -> LightingRequest {
    LightingRequest::ZoneRgb {
        brightness: 3,
        effect,
        all_color: Rgb { r: 255, g: 0, b: 0 },
        zones: [
            Zone {
                enabled: true,
                color: Rgb {
                    r: 0,
                    g: 255,
                    b: 127,
                },
            },
            Zone {
                enabled: false,
                color: Rgb { r: 0, g: 0, b: 255 },
            },
            Zone {
                enabled: true,
                color: Rgb { r: 255, g: 0, b: 0 },
            },
        ],
    }
}
fn words(io: &MockIo) -> Vec<u32> {
    io.words
        .lock()
        .unwrap()
        .iter()
        .map(|(command, word)| {
            assert_eq!(*command, CMD_RGB);
            *word
        })
        .collect()
}
#[test]
fn nine_effects_match_oem_wire_constants_and_persist_modes() {
    let fixtures = [
        (Effect::Random, 0, vec![0xF400008F, 0x70000000, 0xE005B001]),
        (
            Effect::Zones,
            1,
            vec![0xF400008F, 0xF06400FF, 0xF1C80000, 0xF200FF00, 0xE005B001],
        ),
        (
            Effect::Breath,
            2,
            vec![
                0xF400008F, 0xF06400FF, 0xF1C80000, 0xF200FF00, 0x1002A000, 0xE005B001,
            ],
        ),
        (Effect::Cycle, 3, vec![0xF400008F, 0x33010000, 0xE005B001]),
        (Effect::Wave, 4, vec![0xF400008F, 0xB0000000, 0xE005B001]),
        (Effect::Dance, 5, vec![0xF400008F, 0x80000000, 0xE005B001]),
        (Effect::Tempo, 6, vec![0xF400008F, 0x90000000, 0xE005B001]),
        (Effect::Flash, 7, vec![0xF400008F, 0xA0000000, 0xE005B001]),
        (
            Effect::Solid,
            8,
            vec![0xF400008F, 0xF000FF00, 0xF100FF00, 0xF200FF00, 0xE005B001],
        ),
    ];
    for (effect, mode, expected) in fixtures {
        let io = io(preset());
        let outcome = LightingController::default().apply_io(&io, &mut None, &rgb(effect), Some(2));
        assert!(outcome.ok, "{}", outcome.message);
        assert_eq!(words(&io), expected, "{effect:?}");
        let page = io.page.lock().unwrap();
        assert_eq!(page[32], mode);
        assert_eq!(&page[84..88], &[1, 0, 255, 127]);
        assert_eq!(&page[88..92], &[0, 0, 0, 255]);
        assert_eq!(&page[96..104], &preset()[96..104]);
    }
}
#[test]
fn special_type_encoding_and_recovery_retain_original_effect() {
    for raw_type in [6, 22] {
        let io = io(preset());
        let mut baseline = None;
        assert!(
            LightingController::default()
                .apply_io(&io, &mut baseline, &rgb(Effect::Breath), Some(raw_type))
                .ok
        );
        assert_eq!(words(&io)[1..4], [0xF04600FF, 0xF1FF0000, 0xF200FF00]);
        io.words.lock().unwrap().clear();
        assert!(transaction::restore_initial(&io, &baseline).ok);
        assert_eq!(words(&io), [0xF40000BF, 0xB0000000, 0xE005B001]);
        assert_eq!(*io.page.lock().unwrap(), preset());
    }
}
#[test]
fn every_immediate_failure_restores_previous_success_and_initial_state_remains_available() {
    for fail in 0..6 {
        let io = io(preset());
        let mut baseline = None;
        let controller = LightingController::default();
        assert!(
            controller
                .apply_io(&io, &mut baseline, &rgb(Effect::Cycle), Some(2))
                .ok
        );
        let before = *io.page.lock().unwrap();
        io.words.lock().unwrap().clear();
        io.fail_at.set(Some(fail));
        let outcome = controller.apply_io(&io, &mut baseline, &rgb(Effect::Breath), Some(2));
        assert!(!outcome.ok);
        assert!(outcome.rollback_error.is_none());
        assert_eq!(*io.page.lock().unwrap(), before);
        assert!(words(&io).ends_with(&[0xF400008F, 0x33010000, 0xE005B001]));
        assert!(transaction::restore_initial(&io, &baseline).ok);
        assert_eq!(*io.page.lock().unwrap(), preset());
    }
}
#[test]
fn partial_saved_failures_restore_fields_and_previous_dynamic_effect() {
    for fail in 0..7 {
        let io = io(preset());
        io.fail_saved_at.set(Some(fail));
        let mut baseline = None;
        let outcome = LightingController::default().apply_io(
            &io,
            &mut baseline,
            &rgb(Effect::Solid),
            Some(2),
        );
        assert!(!outcome.ok);
        assert!(outcome.rollback_error.is_none());
        assert_eq!(*io.page.lock().unwrap(), preset());
        assert!(words(&io).ends_with(&[0xF40000BF, 0xB0000000, 0xE005B001]));
        assert!(transaction::restore_initial(&io, &baseline).ok);
    }
}
#[test]
fn disabled_baseline_keeps_brightness_setting_but_replays_off() {
    let mut page = preset();
    page[80] = 0;
    let io = io(page);
    let mut baseline = None;
    assert!(
        LightingController::default()
            .apply_io(&io, &mut baseline, &rgb(Effect::Flash), Some(2))
            .ok
    );
    io.words.lock().unwrap().clear();
    assert!(transaction::restore_initial(&io, &baseline).ok);
    assert_eq!(words(&io), [0xF4000000, 0xB0000000, 0xE0000007]);
    assert_eq!(*io.page.lock().unwrap(), page);
}
#[test]
fn invalid_requests_and_unrestorable_baselines_never_write() {
    let controller = LightingController::default();
    let bad_request = LightingRequest::White { brightness: 6 };
    let io = io(preset());
    assert!(
        !controller
            .apply_io(&io, &mut None, &bad_request, Some(1))
            .ok
    );
    assert!(words(&io).is_empty());
    for (offset, bad) in [(32, 9), (35, 8)] {
        let mut page = preset();
        page[offset] = bad;
        let io = super::tests::io(page);
        let mut baseline = None;
        assert!(
            !controller
                .apply_io(&io, &mut baseline, &rgb(Effect::Solid), Some(2))
                .ok
        );
        assert!(words(&io).is_empty());
        assert!(baseline.is_none());
    }
    io.fail_read.set(true);
    assert!(
        !controller
            .apply_io(&io, &mut None, &rgb(Effect::Zones), Some(2))
            .ok
    );
    assert!(words(&io).is_empty());
    assert!(!transaction::restore_initial(&io, &None).ok);
}
#[test]
fn current_request_contract_rejects_legacy_fields_unknown_effect_and_short_zone_array() {
    let request = serde_json::to_value(rgb(Effect::Breath)).unwrap();
    let decoded: LightingRequest = serde_json::from_value(request.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), request);
    for field in ["color", "leftColor"] {
        let mut bad = request.clone();
        bad[field] = serde_json::json!({"r":1,"g":2,"b":3});
        assert!(serde_json::from_value::<LightingRequest>(bad).is_err());
    }
    let mut bad = request.clone();
    bad["effect"] = "unsupported".into();
    assert!(serde_json::from_value::<LightingRequest>(bad).is_err());
    let mut bad = request;
    bad["zones"].as_array_mut().unwrap().pop();
    assert!(serde_json::from_value::<LightingRequest>(bad).is_err());
}
#[test]
fn family_and_encoding_cannot_change_after_baseline_capture() {
    let io = io(preset());
    let mut baseline = None;
    let controller = LightingController::default();
    assert!(
        controller
            .apply_io(&io, &mut baseline, &rgb(Effect::Zones), Some(2))
            .ok
    );
    io.words.lock().unwrap().clear();
    assert!(
        !controller
            .apply_io(
                &io,
                &mut baseline,
                &LightingRequest::White { brightness: 2 },
                Some(1)
            )
            .ok
    );
    assert!(
        !controller
            .apply_io(&io, &mut baseline, &rgb(Effect::Solid), Some(6))
            .ok
    );
    assert!(words(&io).is_empty());
}
#[test]
fn white_backlight_uses_only_cmd39_and_restores_exact_saved_block() {
    let io = io(preset());
    let mut baseline = None;
    assert!(
        LightingController::default()
            .apply_io(
                &io,
                &mut baseline,
                &LightingRequest::White { brightness: 2 },
                Some(1)
            )
            .ok
    );
    assert_eq!(*io.words.lock().unwrap(), [(CMD_WHITE, 2)]);
    assert!(transaction::restore_initial(&io, &baseline).ok);
    assert_eq!(*io.words.lock().unwrap(), [(CMD_WHITE, 2), (CMD_WHITE, 4)]);
    assert_eq!(*io.page.lock().unwrap(), preset());
}
#[test]
fn detection_priority_does_not_guess_rgb() {
    assert_eq!(family_from_type(1), KeyboardFamily::White);
    assert_eq!(family_from_type(99), KeyboardFamily::Unknown);
    assert_eq!(select_detection(Some(2), None).1, KeyboardFamily::ZoneRgb);
    assert_eq!(select_detection(Some(0), Some(22)).0, Some(22));
    assert_eq!(select_detection(None, None).1, KeyboardFamily::Unknown);
}

#[test]
fn zone_restore_keeps_current_sleep_perkey_and_peripheral_values() {
    let io = io(preset());
    let mut baseline = None;
    assert!(
        LightingController::default()
            .apply_io(&io, &mut baseline, &rgb(Effect::Solid), Some(2))
            .ok
    );
    let changes = [
        (33, 1),
        (34, 2),
        (36, 1),
        (37, 1),
        (41, 1),
        (96, 0),
        (100, 0),
        (101, 99),
        (104, 1),
        (105, 87),
    ];
    for (at, value) in changes {
        io.page.lock().unwrap()[at] = value;
    }
    io.words.lock().unwrap().clear();
    assert!(transaction::restore_initial(&io, &baseline).ok);
    for (at, value) in changes {
        assert_eq!(io.page.lock().unwrap()[at], value);
    }
    assert_eq!(*words(&io).last().unwrap(), 0xE0051007);
}

#[test]
fn shared_bit15_uses_light_bar_status_when_that_device_is_declared() {
    let mut page = preset();
    page[96] = 1;
    page[104] = 0;
    let block = SavedBlock::from_bytes(page[32..108].to_vec()).unwrap();
    let zones = block.snapshot().zones;
    assert_eq!(
        encode_onoff_mask(&block, &zones, true, false) & (1 << 15),
        1 << 15
    );
    assert_eq!(encode_onoff_mask(&block, &zones, true, true) & (1 << 15), 0);
}

#[test]
fn changed_peripheral_capability_rejects_both_raw_and_saved_initial_restore() {
    let io = io(preset());
    let mut baseline = None;
    assert!(
        LightingController::default()
            .apply_io(&io, &mut baseline, &rgb(Effect::Solid), Some(2))
            .ok
    );
    io.words.lock().unwrap().clear();
    let saved_writes = io.saved_writes.get();
    io.light_bar.set(true);
    assert!(!transaction::restore_initial(&io, &baseline).ok);
    assert!(words(&io).is_empty());
    assert_eq!(io.saved_writes.get(), saved_writes);
}

#[test]
fn recovery_error_remains_visible_and_saved_restore_is_independent_and_retryable() {
    let io = io(preset());
    let mut baseline = None;
    let controller = LightingController::default();
    assert!(
        controller
            .apply_io(&io, &mut baseline, &rgb(Effect::Solid), Some(2))
            .ok
    );
    io.fail_all_raw.set(true);
    let failed = controller.apply_io(&io, &mut baseline, &rgb(Effect::Breath), Some(2));
    assert!(!failed.ok);
    assert!(failed.rollback_error.unwrap().contains("持续故障"));
    let restored = transaction::restore_initial(&io, &baseline);
    assert!(!restored.ok);
    assert!(restored.message.contains("持续故障"));
    assert_eq!(*io.page.lock().unwrap(), preset());
    assert!(baseline.is_some());
    io.fail_all_raw.set(false);
    assert!(transaction::restore_initial(&io, &baseline).ok);
    assert_eq!(*io.page.lock().unwrap(), preset());
}
