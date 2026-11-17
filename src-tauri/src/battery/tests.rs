use super::*;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

struct Fake {
    state: RefCell<Option<ChargeState>>,
    power: RefCell<PowerStatus>,
    sent: RefCell<Vec<ChargeValues>>,
    fail: RefCell<BTreeSet<usize>>,
    drop: RefCell<BTreeSet<usize>>,
    power_error: Cell<bool>,
    charge_error: Cell<bool>,
}
impl Fake {
    fn new() -> Self {
        Self {
            state: RefCell::new(protocol::charge(&packet()).unwrap()),
            power: RefCell::new(PowerStatus {
                present: Some(true),
                percent: Some(60),
                ..Default::default()
            }),
            sent: Default::default(),
            fail: Default::default(),
            drop: Default::default(),
            power_error: Cell::new(false),
            charge_error: Cell::new(false),
        }
    }
    fn request(&self, enabled: bool, start: u8, stop: u8) -> ChargeRequest {
        ChargeRequest {
            expected_revision: current(self).unwrap().revision(),
            values: ChargeValues {
                enabled,
                start,
                stop,
            },
        }
    }
}
impl Io for Fake {
    fn charge(&self) -> Result<Option<ChargeState>> {
        if self.charge_error.get() {
            bail!("隔离固件读取失败");
        }
        Ok(self.state.borrow().clone())
    }
    fn send(&self, values: &ChargeValues) -> Result<()> {
        self.sent.borrow_mut().push(values.clone());
        let step = self.sent.borrow().len();
        if !self.drop.borrow().contains(&step) {
            self.state.borrow_mut().as_mut().unwrap().current = values.clone();
        }
        if self.fail.borrow().contains(&step) {
            bail!("隔离部分写入失败{step}");
        }
        Ok(())
    }
    fn power(&self) -> Result<PowerStatus> {
        if self.power_error.get() {
            bail!("隔离Windows电源读取失败");
        }
        Ok(self.power.borrow().clone())
    }
    fn diagnostics(&self) -> Result<Option<Diagnostics>> {
        Ok(None)
    }
}
fn packet() -> [u8; 256] {
    let mut raw = [0; 256];
    raw[..3].copy_from_slice(&[0, 95, 100]);
    raw[16..22].copy_from_slice(&[40, 50, 60, 70, 80, 95]);
    raw[32..37].copy_from_slice(&[60, 70, 80, 90, 100]);
    raw
}
#[test]
fn firmware_candidates_and_original_packet_offsets_are_preserved() {
    let decoded = protocol::charge(&packet()).unwrap().unwrap();
    assert_eq!(decoded.start_values, [40, 50, 60, 70, 80, 95]);
    assert_eq!(decoded.stop_values, [60, 70, 80, 90, 100]);
    let encoded = protocol::request(&ChargeValues {
        enabled: true,
        start: 70,
        stop: 80,
    });
    assert_eq!(&encoded[..4], &[31, 1, 70, 80]);
    assert!(encoded[4..].iter().all(|v| *v == 0));
    assert!(protocol::charge(&[0; 256]).unwrap().is_none());
}
#[test]
fn malformed_firmware_does_not_open_policy_controls() {
    for offset in [0, 1, 2, 16, 32] {
        let mut raw = packet();
        raw[offset] = 255;
        assert!(protocol::charge(&raw).is_err());
    }
    let mut raw = packet();
    raw[16..32].fill(0);
    assert!(protocol::charge(&raw).is_err());
    let mut raw = packet();
    raw[32..48].fill(0);
    assert!(protocol::charge(&raw).is_err());
}
#[test]
fn apply_changes_firmware_after_preview_and_restores_initial_values() {
    let io = Fake::new();
    let initial = current(&io).unwrap();
    let mut base = None;
    let request = io.request(true, 70, 80);
    apply(&io, &mut base, &request).unwrap();
    assert_eq!(current(&io).unwrap().current, request.values);
    assert_eq!(base, Some(initial.clone()));
    restore(&io, &mut base, &current(&io).unwrap().revision()).unwrap();
    assert_eq!(current(&io).unwrap(), initial);
    assert!(base.is_some());
}
#[test]
fn out_of_candidates_and_inverted_thresholds_never_send() {
    let io = Fake::new();
    for (start, stop) in [(69, 80), (70, 81), (80, 80), (95, 60), (0, 100)] {
        assert!(apply(&io, &mut None, &io.request(true, start, stop)).is_err());
    }
    assert!(io.sent.borrow().is_empty());
}
#[test]
fn stale_preview_and_capability_changes_refuse_before_write() {
    let io = Fake::new();
    let request = io.request(true, 70, 80);
    io.state.borrow_mut().as_mut().unwrap().current.start = 80;
    assert!(apply(&io, &mut None, &request).is_err());
    assert!(io.sent.borrow().is_empty());
    let mut base = Some(current(&io).unwrap());
    io.state.borrow_mut().as_mut().unwrap().stop_values.pop();
    let expected = current(&io).unwrap().revision();
    assert!(restore(&io, &mut base, &expected).is_err());
    assert!(io.sent.borrow().is_empty());
}
#[test]
fn missing_battery_and_unknown_power_fail_without_sending() {
    let io = Fake::new();
    for present in [Some(false), None] {
        io.power.borrow_mut().present = present;
        assert!(apply(&io, &mut None, &io.request(true, 70, 80)).is_err());
    }
    io.power_error.set(true);
    assert!(apply(&io, &mut None, &io.request(true, 70, 80)).is_err());
    assert!(io.sent.borrow().is_empty());
}
#[test]
fn partial_write_and_readback_mismatch_compensate_to_actual_before() {
    for partial in [true, false] {
        let io = Fake::new();
        let initial = current(&io).unwrap();
        let mut base = None;
        if partial {
            io.fail.borrow_mut().insert(1);
        } else {
            io.drop.borrow_mut().insert(1);
        }
        let error = apply(&io, &mut base, &io.request(true, 70, 80)).unwrap_err();
        assert!(error.to_string().contains("充电设置失败"));
        assert_eq!(current(&io).unwrap(), initial);
        assert_eq!(io.sent.borrow().len(), 2);
        assert!(base.is_some());
    }
}
#[test]
fn failed_compensation_retains_baseline_for_explicit_retry() {
    let io = Fake::new();
    let initial = current(&io).unwrap();
    let mut base = None;
    io.fail.borrow_mut().insert(1);
    io.drop.borrow_mut().insert(2);
    let error = apply(&io, &mut base, &io.request(true, 70, 80))
        .unwrap_err()
        .to_string();
    assert!(error.contains("恢复失败"));
    assert_ne!(current(&io).unwrap(), initial);
    io.fail.borrow_mut().clear();
    io.drop.borrow_mut().clear();
    restore(&io, &mut base, &current(&io).unwrap().revision()).unwrap();
    assert_eq!(current(&io).unwrap(), initial);
}
#[test]
fn later_failure_preserves_first_baseline_and_successful_previous_policy() {
    let io = Fake::new();
    let initial = current(&io).unwrap();
    let mut base = None;
    apply(&io, &mut base, &io.request(true, 70, 80)).unwrap();
    let previous = current(&io).unwrap();
    io.fail.borrow_mut().insert(2);
    assert!(apply(&io, &mut base, &io.request(true, 80, 90)).is_err());
    assert_eq!(current(&io).unwrap(), previous);
    assert_eq!(base, Some(initial));
}
#[test]
fn disabled_zero_threshold_baseline_restores_exactly_without_invented_defaults() {
    let io = Fake::new();
    io.state.borrow_mut().as_mut().unwrap().current = ChargeValues {
        enabled: false,
        start: 0,
        stop: 0,
    };
    let initial = current(&io).unwrap();
    let mut base = None;
    apply(&io, &mut base, &io.request(true, 70, 80)).unwrap();
    restore(&io, &mut base, &current(&io).unwrap().revision()).unwrap();
    assert_eq!(current(&io).unwrap(), initial);
}
#[test]
fn unsupported_and_noop_do_not_claim_a_restorable_write() {
    let io = Fake::new();
    let mut base = None;
    apply(&io, &mut base, &io.request(false, 95, 100)).unwrap();
    assert!(base.is_none());
    *io.state.borrow_mut() = None;
    assert!(
        apply(
            &io,
            &mut base,
            &ChargeRequest {
                expected_revision: "".into(),
                values: ChargeValues {
                    enabled: true,
                    start: 70,
                    stop: 80
                }
            }
        )
        .is_err()
    );
    assert!(io.sent.borrow().is_empty());
}
#[test]
fn profile_keeps_windows_readings_when_oem_transport_fails() {
    let io = Fake::new();
    io.charge_error.set(true);
    let p = read(&io, &None);
    assert!(p.charge.is_none());
    assert!(p.revision.is_none());
    assert_eq!(p.power.unwrap().percent, Some(60));
    assert!(p.warnings[0].contains("固件读取失败"));
}
#[test]
fn windows_unknown_is_never_misreported_as_empty_or_discharging() {
    use windows_sys::Win32::System::Power::SYSTEM_POWER_STATUS;
    let raw = SYSTEM_POWER_STATUS {
        ACLineStatus: 255,
        BatteryFlag: 255,
        BatteryLifePercent: 255,
        SystemStatusFlag: 255,
        BatteryLifeTime: u32::MAX,
        ..Default::default()
    };
    assert_eq!(native::power(&raw), PowerStatus::default());
    let absent = SYSTEM_POWER_STATUS {
        BatteryFlag: 128,
        BatteryLifePercent: 0,
        ..raw
    };
    let p = native::power(&absent);
    assert_eq!(p.present, Some(false));
    assert_eq!(p.percent, None);
    assert_eq!(p.charging, None);
    let present = SYSTEM_POWER_STATUS {
        ACLineStatus: 0,
        BatteryFlag: 8,
        BatteryLifePercent: 0,
        BatteryLifeTime: 42,
        ..raw
    };
    let p = native::power(&present);
    assert_eq!(p.percent, Some(0));
    assert_eq!(p.charging, Some(true));
    assert_eq!(p.remaining_seconds, Some(42));
}
#[test]
fn diagnostic_fields_keep_all_32_bits_and_capacity_units_unknown() {
    let mut raw = [0; 256];
    raw[6..8].copy_from_slice(&6000u16.to_le_bytes());
    raw[8..10].copy_from_slice(&5000u16.to_le_bytes());
    raw[12..16].copy_from_slice(&0xAABBCCDDu32.to_le_bytes());
    raw[16..20].copy_from_slice(&0x11223344u32.to_le_bytes());
    raw[2..4].copy_from_slice(&(((2024 - 1980) << 9 | 2 << 5 | 29) as u16).to_le_bytes());
    let d = protocol::diagnostics(&raw);
    assert_eq!(d.capacity_pct, Some(120.0));
    assert_eq!(d.manufactured_on.as_deref(), Some("2024-02-29"));
    assert_eq!(d.permanent_failure_status, 0xAABBCCDD);
    assert_eq!(d.operation_status, 0x11223344);
    raw[8..10].fill(0);
    raw[2..4].copy_from_slice(&(((2023 - 1980) << 9 | 2 << 5 | 29) as u16).to_le_bytes());
    let d = protocol::diagnostics(&raw);
    assert!(d.capacity_pct.is_none());
    assert!(d.manufactured_on.is_none());
}
#[test]
fn utility_capability_uses_extended_wmi16_not_integer_echo() {
    let mut caps = [0; 15];
    caps[0] = 1;
    caps[14] = 2;
    assert!(protocol::utility_supported(&caps).unwrap());
    caps[14] = 1;
    assert!(!protocol::utility_supported(&caps).unwrap());
    caps[0] = 0;
    assert!(!protocol::utility_supported(&caps).unwrap());
    caps[0] = 2;
    assert!(protocol::utility_supported(&caps).is_err());
}
#[test]
fn requests_reject_old_or_extra_fields() {
    assert!(
        serde_json::from_str::<ChargeRequest>(
            r#"{"expectedRevision":"a","values":{"enabled":true,"start":70,"stop":80,"mode":1}}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<ChargeRequest>(
            r#"{"expectedRevision":"a","values":{"enabled":true,"start":70,"stop":80},"mode":1}"#
        )
        .is_err()
    );
}
