use super::*;

fn limits() -> Limits {
    let mut packet = [0u8; 256];
    for (i, value) in [1400u16, 1700, 900, 700, 650].into_iter().enumerate() {
        packet[2 + i * 2..4 + i * 2].copy_from_slice(&value.to_le_bytes());
    }
    Limits::from_packet(&packet)
}

fn saved() -> [Offsets; 2] {
    Offsets::saved(&[100, 0, 0xfe, 0xff, 1, 0, 0xff, 0xff]).unwrap()
}

#[test]
fn exact_original_capability_versions_and_fallback() {
    let mut packet = [0u8; 256];
    packet[13] = 1;
    assert!(Capabilities::from_packet(&packet, None).is_err());
    assert_eq!(
        Capabilities::from_packet(&packet, Some(0x20)).unwrap(),
        Capabilities {
            limit_clock: true,
            all_mode_oc: true
        }
    );
    packet = [0; 256];
    packet[0] = 1;
    packet[15] = 1;
    assert!(Capabilities::from_packet(&packet, None).is_err());
    packet[10] = 0x20;
    assert_eq!(
        Capabilities::from_packet(&packet, None).unwrap(),
        Capabilities {
            limit_clock: true,
            all_mode_oc: true
        }
    );
    packet[0] = 2;
    assert!(Capabilities::from_packet(&packet, Some(0)).is_err());
}

#[test]
fn v1_zero_sum_uses_fallback_even_when_individual_fields_are_nonzero() {
    let mut packet = [0u8; 256];
    packet[0] = 1;
    packet[2..6].copy_from_slice(&u32::MAX.to_le_bytes());
    packet[10] = 1;
    assert!(Capabilities::from_packet(&packet, None).is_err());
    assert!(
        Capabilities::from_packet(&packet, Some(0x20))
            .unwrap()
            .all_mode_oc
    );
}

#[test]
fn original_ones_complement_offsets_are_not_twos_complement() {
    assert_eq!(
        saved(),
        [
            Offsets {
                core_mhz: 100,
                memory_mhz: -1
            },
            Offsets {
                core_mhz: 1,
                memory_mhz: 0
            }
        ]
    );
    assert!(Offsets::saved(&[0; 7]).is_err());
}

#[test]
fn turbo_off_uses_thermal_or_actual_base_without_touching_offsets() {
    let caps = Capabilities {
        limit_clock: false,
        all_mode_oc: false,
    };
    assert_eq!(
        target(96, 255, None, caps, limits(), saved(), 1, None).unwrap(),
        Target {
            lock_mhz: Some(650),
            offsets: None
        }
    );
    let limits = Limits {
        thermal_mhz: 0,
        ..limits()
    };
    assert!(target(96, 0, None, caps, limits, saved(), 1, None).is_err());
    assert!(target(96, 0, None, caps, limits, saved(), 1, Some(0)).is_err());
    assert_eq!(
        target(96, 0, None, caps, limits, saved(), 1, Some(1350))
            .unwrap()
            .lock_mhz,
        Some(1350)
    );
}

#[test]
fn turbo_on_obeys_mode_supply_capabilities_and_at_most_two_gpus() {
    for limit_clock in [false, true] {
        for all_mode_oc in [false, true] {
            let caps = Capabilities {
                limit_clock,
                all_mode_oc,
            };
            for (mode, expected) in [(0, 700), (1, 900), (2, 1700), (3, 1400)] {
                let result =
                    target(97, mode, Some(true), caps, limits(), saved(), 3, None).unwrap();
                assert_eq!(
                    result.lock_mhz,
                    Some(if limit_clock { expected } else { 0 })
                );
                let offsets = result.offsets.unwrap();
                assert_eq!(offsets.len(), 2);
                assert_eq!(
                    offsets,
                    if mode == 2 || all_mode_oc {
                        saved().to_vec()
                    } else {
                        vec![Offsets::ZERO; 2]
                    }
                );
            }
            let result = target(97, 2, Some(false), caps, limits(), saved(), 1, None).unwrap();
            assert_eq!(
                result,
                Target {
                    lock_mhz: None,
                    offsets: Some(vec![Offsets::ZERO])
                }
            );
        }
    }
}

#[test]
fn unknown_mode_event_supply_and_no_gpu_are_rejected() {
    let caps = Capabilities {
        limit_clock: true,
        all_mode_oc: true,
    };
    assert_eq!(
        target(97, 4, Some(true), caps, limits(), saved(), 1, None).unwrap(),
        Target {
            lock_mhz: None,
            offsets: None
        }
    );
    assert!(target(97, 5, Some(true), caps, limits(), saved(), 1, None).is_err());
    assert!(target(97, 2, None, caps, limits(), saved(), 1, None).is_err());
    assert!(target(95, 0, Some(true), caps, limits(), saved(), 1, None).is_err());
    assert!(target(97, 0, Some(true), caps, limits(), saved(), 0, None).is_err());
}
