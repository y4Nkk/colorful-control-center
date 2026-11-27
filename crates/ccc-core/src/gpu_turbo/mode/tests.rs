use super::*;

fn packet() -> [u8; 256] {
    let mut packet = [0; 256];
    packet[0] = 1;
    packet[2] = 1;
    packet
}

fn policy(packet: &[u8; 256], wmi16: Option<i32>, model: u32, customer: u8) -> Policy {
    let mut cache = [0; 13];
    cache[6] = 2;
    cache[9] = 1;
    Policy::from_packet(
        packet,
        wmi16,
        Some(0),
        model,
        customer,
        &cache,
        Limits {
            entertainment_mhz: 1400,
            performance_mhz: 1700,
            saving_mhz: 900,
            quiet_mhz: 700,
            thermal_mhz: 650,
        },
        [Offsets {
            core_mhz: 100,
            memory_mhz: -1,
        }; 2],
    )
    .unwrap()
}

#[test]
fn versioned_gpu_oc_bits_and_missing_legacy_values_remain_distinct() {
    let mut embedded = packet();
    embedded[13] = 0x20;
    let source = policy(&embedded, Some(0), 1, 5);
    assert_eq!(source.gpu_oc, Some(true));
    assert!(source.supported().unwrap());
    let legacy = policy(&[0; 256], None, 1, 5);
    assert_eq!(legacy.gpu_oc, None);
    assert!(legacy.decide(2, Some(true), false).is_err());
    assert_eq!(policy(&[0; 256], Some(0), 1, 5).supported(), Ok(false));
    let mut empty = [0; 256];
    empty[0] = 1;
    assert_eq!(policy(&empty, Some(0x20), 1, 5).gpu_oc, Some(true));
}

#[test]
fn simple_oc_model_overrides_preserve_the_customer_five_exception() {
    for model in [
        343, 344, 375, 376, 14081, 14082, 26273, 26274, 26275, 42576, 42577, 42593, 42609, 42610,
        54529, 54530, 55041, 55042,
    ] {
        assert!(policy(&packet(), None, model, 7).simple_gpu_oc);
        assert!(!policy(&packet(), None, model, 5).simple_gpu_oc);
    }
    assert!(!policy(&packet(), None, 1, 7).simple_gpu_oc);
    let mut advertised = packet();
    advertised[20] = 8;
    assert!(policy(&advertised, None, 1, 5).simple_gpu_oc);
    let mut legacy = [0; 256];
    legacy[20] = 8;
    assert!(!policy(&legacy, Some(0), 1, 5).simple_gpu_oc);
}

#[test]
fn resume_and_not_ready_are_explicit_non_writing_decisions() {
    let mut source = policy(&packet(), None, 343, 7);
    assert_eq!(
        source.decide(2, Some(true), true).unwrap().skipped,
        Some(SkipReason::Resume)
    );
    source.ready = false;
    source.gpu_count = 0;
    assert_eq!(
        source.decide(2, None, false).unwrap().skipped,
        Some(SkipReason::NotReady)
    );
    source.ready = true;
    assert!(source.decide(2, Some(true), false).is_err());
    source.gpu_count = 65;
    assert!(source.decide(2, Some(true), false).is_err());
    assert!(source.decide(4, Some(true), true).is_err());
    let mut unknown = policy(&[0; 256], None, 1, 5);
    unknown.ready = false;
    assert_eq!(
        unknown.decide(2, None, false).unwrap().skipped,
        Some(SkipReason::NotReady)
    );
}

#[test]
fn all_mode_targets_reuse_original_turbo_rules_and_do_not_require_thermal_base_clock() {
    let mut packet = packet();
    packet[10] = 0x20;
    packet[15] = 1;
    let mut source = policy(&packet, None, 1, 5);
    source.limits.thermal_mhz = 0;
    for mode in 0..crate::oem_events::POWER_MODE_COUNT {
        for on_ac in [true, false] {
            let decision = source.decide(mode, Some(on_ac), false).unwrap();
            assert_eq!(
                decision.target.unwrap(),
                target(
                    97,
                    mode,
                    Some(on_ac),
                    source.capabilities.unwrap(),
                    source.limits,
                    source.saved,
                    2,
                    None
                )
                .unwrap()
            );
        }
    }
    let dc = source
        .decide(2, Some(false), false)
        .unwrap()
        .target
        .unwrap();
    assert_eq!(dc.lock_mhz, None);
    assert_eq!(dc.offsets.unwrap(), vec![Offsets::ZERO; 2]);
}

#[test]
fn unknown_wmi96_remains_unknown_and_does_not_block_a_non_writing_mode() {
    let known = policy(&[0; 256], Some(0), 1, 5);
    let source = Policy::from_packet(
        &[0; 256],
        None,
        None,
        1,
        5,
        &[0; 13],
        known.limits,
        known.saved,
    )
    .unwrap();
    assert!(source.capabilities.is_none() && source.gpu_oc.is_none());
    assert_eq!(
        source.decide(2, Some(true), false).unwrap().skipped,
        Some(SkipReason::NotReady)
    );
    let mut ready = source;
    ready.ready = true;
    ready.gpu_count = 1;
    assert!(
        ready
            .decide(2, Some(true), false)
            .unwrap_err()
            .contains("WMI 96")
    );
    assert_eq!(
        ready.decide(2, Some(true), true).unwrap().skipped,
        Some(SkipReason::Resume)
    );
}
