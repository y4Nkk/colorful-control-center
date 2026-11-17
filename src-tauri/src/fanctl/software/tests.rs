use super::*;
use crate::hal::NvidiaSnapshot;
use ccc_core::fan::{CurvePoint, FanCurve};

fn curves() -> Curves {
    ["cpu", "gpu1"]
        .map(|id| {
            (
                id.into(),
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
                    ramp_pct_per_s: 100.0,
                },
            )
        })
        .into()
}

fn sample(cpu: Option<f64>, gpu: Option<f64>) -> HardwareSnapshot {
    HardwareSnapshot {
        cpu_temp_c: cpu,
        nvidia: Some(NvidiaSnapshot {
            temp_c: gpu,
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn software_session_outputs_both_channels_using_hottest_sensor() {
    let current = sample(Some(50.0), Some(70.0));
    let mut s = Session::new(curves(), &current).unwrap();
    let mut packets = vec![];
    s.advance(&current, 1.0, |p| {
        packets.push(*p);
        Ok(())
    })
    .unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!((packets[0][1], packets[1][1]), (1, 2));
    assert_eq!(packets[0][2], packets[1][2]);
    assert_eq!(packets[0][6], 0xc1);
}

#[test]
fn gpu_channel_requires_a_live_gpu_before_taking_control() {
    assert!(Session::new(curves(), &sample(Some(50.0), None)).is_err());
    let mut c = curves();
    c.remove("gpu1");
    assert!(Session::new(c, &sample(Some(50.0), None)).is_ok());
}

#[test]
fn lost_sensors_and_fuse_stop_before_any_output() {
    let mut s = Session::new(curves(), &sample(Some(50.0), Some(50.0))).unwrap();
    for reading in [
        sample(None, Some(50.0)),
        sample(Some(50.0), None),
        sample(Some(95.0), Some(50.0)),
        sample(Some(50.0), Some(f64::NAN)),
        sample(Some(50.0), Some(95.0)),
    ] {
        assert!(
            s.advance(&reading, 0.5, |_| panic!("失效后不能输出"))
                .is_err()
        );
    }
}

#[test]
fn gpu_appearing_later_is_also_required_afterwards() {
    let mut c = curves();
    c.remove("gpu1");
    let mut s = Session::new(c, &sample(Some(50.0), None)).unwrap();
    s.advance(&sample(Some(50.0), Some(60.0)), 0.5, |_| Ok(()))
        .unwrap();
    assert!(
        s.advance(&sample(Some(50.0), None), 0.5, |_| panic!("GPU 已丢失"))
            .is_err()
    );
}

#[test]
fn first_channel_failure_stops_without_writing_the_second() {
    let current = sample(Some(50.0), Some(50.0));
    let mut s = Session::new(curves(), &current).unwrap();
    let mut writes = 0;
    let error = s
        .advance(&current, 0.5, |_| {
            writes += 1;
            Err(anyhow!("transport failure"))
        })
        .unwrap_err();
    assert_eq!(writes, 1);
    assert!(error.to_string().contains("transport failure"));
}

#[test]
fn late_control_tick_cannot_resume_manual_output() {
    let current = sample(Some(50.0), Some(50.0));
    let mut s = Session::new(curves(), &current).unwrap();
    assert!(
        s.advance(&current, 3.01, |_| panic!("超时后不能输出"))
            .is_err()
    );
}
