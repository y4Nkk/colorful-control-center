use super::*;
use crate::fan::CurvePoint;

fn curves() -> Curves {
    BTreeMap::from([(
        "cpu".into(),
        FanCurve {
            points: vec![
                CurvePoint {
                    temp_c: 40.0,
                    duty_pct: 20.0,
                },
                CurvePoint {
                    temp_c: 95.0,
                    duty_pct: 100.0,
                },
            ],
            hysteresis_c: 3.0,
            ramp_pct_per_s: 10.0,
        },
    )])
}

#[test]
fn exact_c1_packet_only_addresses_one_channel() {
    // 手动 C1(channel, duty) 与自动 C1(0xFF, channel) 参数顺序不同。
    assert_eq!(&duty_packet(1, 100.0).unwrap()[1..3], &[1, 255]);
    for channel in 1..=4 {
        let p = duty_packet(channel, 50.0).unwrap();
        assert_eq!(&p[..7], &[1, channel, 128, 0, 0, 0, 0xc1]);
        assert!(p[7..].iter().all(|v| *v == 0));
    }
    for (channel, pct) in [(0, 50.0), (5, 50.0), (1, 0.0), (1, 101.0), (1, f64::NAN)] {
        assert!(duty_packet(channel, pct).is_err());
    }
}

#[test]
fn input_contract_rejects_invalid_shapes_instead_of_normalizing() {
    let valid = curves();
    assert!(validate(&valid).is_ok());
    for fault in 0..9 {
        let mut c = valid.clone();
        let fan = c.get_mut("cpu").unwrap();
        match fault {
            0 => {
                fan.points.pop();
            }
            1 => fan.points[1].temp_c = 40.0,
            2 => fan.points[1].duty_pct = 90.0,
            3 => fan.points[0].duty_pct = 0.0,
            4 => fan.points[0].temp_c = f64::NAN,
            5 => fan.hysteresis_c = 11.0,
            6 => fan.ramp_pct_per_s = 0.0,
            7 => fan.points = vec![fan.points[0]; 13],
            _ => {
                let fan = fan.clone();
                c.clear();
                c.insert("unknown".into(), fan);
            }
        }
        assert!(validate(&c).is_err(), "fault {fault}");
    }
    assert!(validate(&BTreeMap::new()).is_err());
}

#[test]
fn ramp_limits_actual_tick_duration_without_changing_other_channels() {
    let mut c = Controller::new(curves(), &BTreeMap::from([("cpu".into(), 20.0)])).unwrap();
    assert_eq!(c.step(90.0, 0.5).unwrap()[0][2], oem_duty_byte(25.0));
    assert_eq!(c.step(90.0, 1.0).unwrap()[0][2], oem_duty_byte(35.0));
}

#[test]
fn invalid_or_overheated_input_cannot_produce_manual_packets() {
    let mut c = Controller::new(curves(), &BTreeMap::new()).unwrap();
    for (temp, dt) in [
        (95.0, 0.5),
        (96.0, 0.5),
        (f64::NAN, 0.5),
        (60.0, 3.01),
        (60.0, f64::NAN),
    ] {
        assert!(c.step(temp, dt).is_err());
    }
}

#[test]
fn hysteresis_holds_small_cooling_then_allows_decrease() {
    let mut config = curves();
    config.get_mut("cpu").unwrap().ramp_pct_per_s = 100.0;
    let mut c = Controller::new(config, &BTreeMap::from([("cpu".into(), 100.0)])).unwrap();
    let first = c.step(70.0, 1.0).unwrap()[0][2];
    assert_eq!(c.step(69.0, 1.0).unwrap()[0][2], first);
    assert!(c.step(66.0, 1.0).unwrap()[0][2] < first);
}
