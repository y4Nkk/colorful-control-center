use super::*;
use std::sync::Mutex;

struct State {
    power: u64,
    voltage: [i16; 2],
    response: u64,
    writes: Vec<(u32, u64)>,
    ignore_plane: Option<u64>,
    fail_restore: bool,
}
struct Io(Mutex<State>);
impl Io {
    fn new() -> Self {
        Self(Mutex::new(State {
            power: (801 << 32) | 529,
            voltage: [-51, -26],
            response: 0,
            writes: vec![],
            ignore_plane: None,
            fail_restore: false,
        }))
    }
}
impl MsrAccess for Io {
    fn read(&self, address: u32) -> std::result::Result<u64, String> {
        let state = self.0.lock().unwrap();
        Ok(match address {
            msr::MSR_PKG_POWER_LIMIT => state.power,
            msr::MSR_RAPL_POWER_UNIT => 3,
            _ => state.response,
        })
    }
    fn write(&self, address: u32, value: u64) -> std::result::Result<(), String> {
        let mut state = self.0.lock().unwrap();
        state.writes.push((address, value));
        if address == msr::MSR_PKG_POWER_LIMIT {
            if state.fail_restore && value & 0x7fff == 529 {
                return Err("PL restore injected failure".into());
            }
            state.power = value;
        } else {
            let plane = (value >> 40) & 0xf;
            if (value >> 32) & 0xff == 0x11 && state.ignore_plane != Some(plane) {
                state.voltage[plane as usize] = msr::decode_offset(value);
            }
            state.response = ((state.voltage[plane as usize] as u16 & 0x7ff) as u64) << 21;
        }
        Ok(())
    }
}

fn target() -> Settings {
    Settings {
        pl1_w: Some(75.125),
        pl2_w: Some(100.0),
        core_offset_units: Some(-70),
        gt_offset_units: Some(-30),
    }
}

#[test]
fn captures_exact_voltage_and_fractional_rapl_values() {
    let io = Io::new();
    let target = target();
    let before = capture(&io, &target).unwrap();
    assert_eq!(before.voltage[&0], -51);
    commit(&io, &target).unwrap();
    assert_eq!(
        msr::decode_power_limits(io.0.lock().unwrap().power, 0.125),
        (75.125, 100.0)
    );
    restore(&io, &before).unwrap();
    let state = io.0.lock().unwrap();
    assert_eq!(state.power, (801 << 32) | 529);
    assert_eq!(state.voltage, [-51, -26]);
}

#[test]
fn locked_or_inverted_partial_power_fails_before_target_writes() {
    let io = Io::new();
    io.0.lock().unwrap().power |= 1 << 63;
    assert!(
        capture(&io, &target())
            .unwrap_err()
            .to_string()
            .contains("锁定")
    );
    assert!(io.0.lock().unwrap().writes.is_empty());
    io.0.lock().unwrap().power &= !(1 << 63);
    let t = Settings {
        pl1_w: Some(120.0),
        ..Default::default()
    };
    assert!(capture(&io, &t).is_err());
    assert!(io.0.lock().unwrap().writes.is_empty());
}

#[test]
fn ignored_voltage_is_detected_and_raw_snapshot_restores() {
    let io = Io::new();
    let before = capture(&io, &target()).unwrap();
    io.0.lock().unwrap().ignore_plane = Some(1);
    assert!(
        commit(&io, &target())
            .unwrap_err()
            .to_string()
            .contains("回读不一致")
    );
    io.0.lock().unwrap().ignore_plane = None;
    restore(&io, &before).unwrap();
    assert_eq!(io.0.lock().unwrap().voltage, [-51, -26]);
}

#[test]
fn restore_continues_voltage_after_power_restore_error() {
    let io = Io::new();
    let before = capture(&io, &target()).unwrap();
    commit(&io, &target()).unwrap();
    io.0.lock().unwrap().fail_restore = true;
    assert!(
        restore(&io, &before)
            .unwrap_err()
            .to_string()
            .contains("功耗上限恢复")
    );
    assert_eq!(io.0.lock().unwrap().voltage, [-51, -26]);
}

#[test]
fn runtime_contract_rejects_empty_invalid_or_out_of_range_values() {
    assert!(validate(&Settings::default()).is_err());
    for target in [
        Settings {
            pl1_w: Some(f64::NAN),
            ..Default::default()
        },
        Settings {
            core_offset_units: Some(-155),
            ..Default::default()
        },
        Settings {
            gt_offset_units: Some(1),
            ..Default::default()
        },
    ] {
        assert!(validate(&target).is_err());
    }
    assert!(
        validate(&Settings {
            pl1_w: Some(65.125),
            core_offset_units: Some(-154),
            ..Default::default()
        })
        .is_ok()
    );
}
