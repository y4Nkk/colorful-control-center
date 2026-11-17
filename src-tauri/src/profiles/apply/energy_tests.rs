use super::*;

#[test]
fn complete_profile_mode_uses_common_energy_and_the_same_initial_restore() {
    let f = fixtures::Fixture::new().with_energy(2);
    let wanted = ProfileSettings {
        oem_power_mode: Some(1),
        ..Default::default()
    };
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &wanted).unwrap();
    assert!(steps.iter().all(|step| step.ok));
    let _gate = f.hal.begin_write().unwrap();
    let mode = powerctl::mode::read(&f.hal).unwrap();
    assert_eq!(mode.energy.unwrap().energy_star, Some(1));
    let initial = mode.restore.unwrap();
    assert_eq!(initial.energy.unwrap().energy_star, Some(2));
    powerctl::mode::restore(&f.hal, &initial.revision).unwrap();
    assert_eq!(
        powerctl::mode::read(&f.hal)
            .unwrap()
            .energy
            .unwrap()
            .energy_star,
        Some(2)
    );
}

#[test]
fn later_gpu_failure_restores_the_profile_mode_and_exact_initial_energy() {
    let f = fixtures::Fixture::new().with_energy(2);
    f.device.0.lock().unwrap().fail_gpu = true;
    let mut wanted = settings();
    wanted.oem_power_mode = Some(1);
    wanted.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &wanted).unwrap();
    assert!(steps.iter().any(|step| !step.ok));
    let _gate = f.hal.begin_write().unwrap();
    let mode = powerctl::mode::read(&f.hal).unwrap();
    assert_eq!(mode.current_mode, 3);
    assert_eq!(mode.energy.unwrap().energy_star, Some(2));
    let events = &f.device.0.lock().unwrap().events;
    assert!(events.iter().any(|event| event == "ENERGY:1"));
    assert!(events.iter().any(|event| event == "ENERGY:2"));
}
