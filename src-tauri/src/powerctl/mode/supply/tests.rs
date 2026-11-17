use super::*;

fn evidence() -> Evidence {
    let targets = (0..=4)
        .map(|mode| Target {
            from_mode: None,
            gpu: None,
            revision: None,
            unavailable_reason: None,
            ac_mode: None,
            energy: Some(energy::State {
                hardware_p_state: false,
                energy_star: Some(match mode {
                    0 | 3 => 2,
                    1 | 4 => 1,
                    2 => 0,
                    _ => unreachable!(),
                }),
            }),
            mode,
            turbo: mode == 2,
            dtt: false,
            fan_mode: 0,
            fan_saved_mode: 0,
            fan_offset_pct: 0,
            aspm: None,
            power_plan: None,
        })
        .collect();
    Evidence {
        model: Model {
            product: 20600,
            customer: 1,
            dc_mode_power: true,
        },
        quiet_plus_default: false,
        ac_mode: 2,
        current_mode: 1,
        available: vec![0, 1, 2, 3, 4],
        targets,
        on_ac: Some(false),
        energy: energy::State {
            hardware_p_state: false,
            energy_star: Some(0),
        },
        gpu_supported: false,
        gpu_devices: Vec::new(),
        clocks: Some(crate::gpuctl::turbo::mode::test_policy()),
    }
}
#[test]
fn preview_binds_saved_ac_and_complete_mode_effects() {
    let mut facts = evidence();
    let first = review(&facts, false).unwrap().unwrap();
    assert_eq!(
        (
            first.ac.as_ref().unwrap().mode,
            first.battery.as_ref().unwrap().mode
        ),
        (2, 1)
    );
    assert!(first.ac.as_ref().unwrap().turbo);
    assert_eq!(
        (
            first.ac.as_ref().unwrap().energy.unwrap().energy_star,
            first.battery.as_ref().unwrap().energy.unwrap().energy_star
        ),
        (Some(0), Some(1))
    );
    facts.current_mode = 3;
    facts.on_ac = Some(true);
    let changed = review(&facts, false).unwrap().unwrap();
    assert_eq!(first.identity, changed.identity);
    assert_ne!(first.revision, changed.revision);
    facts.energy.energy_star = Some(2);
    let energy_changed = review(&facts, false).unwrap().unwrap();
    assert_eq!(first.identity, energy_changed.identity);
    assert_ne!(changed.revision, energy_changed.revision);
    facts.ac_mode = 3;
    assert_ne!(
        first.identity,
        review(&facts, false).unwrap().unwrap().identity
    );
}
#[test]
fn special_supply_and_startup_targets_are_not_conflated() {
    let mut facts = evidence();
    facts.model.product = 16577;
    facts.model.customer = 7;
    let view = review(&facts, false).unwrap().unwrap();
    assert_eq!(view.battery.unwrap().mode, 3);
    assert_eq!(view.startup_battery.unwrap().mode, 1);
    facts.model.product = 20720;
    let view = review(&facts, false).unwrap().unwrap();
    assert!(view.ac.is_none() && view.battery.is_none());
    assert_eq!(view.startup_battery.unwrap().mode, 1);
}
#[test]
fn unsupported_or_unavailable_choices_are_explicit() {
    let mut facts = evidence();
    facts.model.dc_mode_power = false;
    assert!(review(&facts, false).unwrap().is_none());
    facts.model.customer = 22;
    assert_eq!(
        review(&facts, false)
            .unwrap()
            .unwrap()
            .battery
            .unwrap()
            .mode,
        3
    );
    facts.model.customer = 1;
    facts.model.dc_mode_power = true;
    facts.ac_mode = 9;
    assert!(review(&facts, false).is_err());
}

#[test]
fn preview_keeps_supply_plan_selection_but_startup_and_resume_only_reload_current() {
    let mut facts = evidence();
    facts.on_ac = Some(true);
    for target in &mut facts.targets {
        target.power_plan = Some(2);
    }
    let view = review(&facts, false).unwrap().unwrap();
    assert_eq!(view.ac.as_ref().unwrap().power_plan, Some(2));
    assert_eq!(view.battery.as_ref().unwrap().power_plan, Some(2));
    assert_eq!(view.startup_ac.as_ref().unwrap().power_plan, None);
    assert_eq!(view.startup_battery.as_ref().unwrap().power_plan, None);
    assert_eq!(view.resume_ac.as_ref().unwrap().power_plan, None);
    assert_eq!(view.resume_battery.as_ref().unwrap().power_plan, None);
}

#[test]
fn all_six_conditions_are_visible_even_when_current_supply_is_unknown() {
    let mut facts = evidence();
    facts.on_ac = None;
    let view = review(&facts, false).unwrap().unwrap();
    assert_eq!(
        view.conditions()
            .map(|target| target.as_ref().unwrap().mode),
        [2, 1, 2, 1, 2, 1]
    );
    assert_eq!(
        (
            view.startup_ac
                .as_ref()
                .unwrap()
                .energy
                .unwrap()
                .energy_star,
            view.startup_battery
                .as_ref()
                .unwrap()
                .energy
                .unwrap()
                .energy_star
        ),
        (Some(0), Some(1))
    );
    assert_eq!(
        (
            view.resume_ac.as_ref().unwrap().energy.unwrap().energy_star,
            view.resume_battery
                .as_ref()
                .unwrap()
                .energy
                .unwrap()
                .energy_star
        ),
        (Some(0), Some(1))
    );
    let value = serde_json::to_value(&view).unwrap();
    assert!(value.get("startup").is_none() && value.get("resume").is_none());
    assert!(value["gpu"].get("startup").is_none() && value["clocks"].get("resume").is_none());
    assert!(value.get("startupAc").is_some() && value.get("resumeBattery").is_some());
}
