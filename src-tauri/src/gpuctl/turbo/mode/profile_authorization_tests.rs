use super::*;
use crate::powerctl::mode::profile::{ApprovalPreview, Grant};

fn approval(h: &Harness, settings: &ProfileSettings) -> Result<ApprovalPreview> {
    let session = context(&h.f, &h.memory, &h.gpu, &h.power).session()?;
    let _gate = h.f.hal.begin_write()?;
    crate::profiles::preview_authorization(&session.context, settings)?
        .ok_or_else(|| anyhow::anyhow!("隔离档案缺少模式"))
}

#[test]
fn stable_grant_survives_its_own_mode_offsets_node_and_explicit_policy_outputs() {
    let h = Harness::new(true);
    let preview = h.preview();
    let before = approval(&h, &preview.file.settings).unwrap();
    let mut grant = before.grant().unwrap();
    assert_eq!(grant.automatic_power_off, Some(vec![false, true]));
    assert!(grant.driver.is_none() && grant.driver_deferred);
    success(&h.apply(&preview).unwrap());
    let after = approval(&h, &preview.file.settings).unwrap();
    assert_ne!(before.target.revision, after.target.revision);
    assert_eq!(before.identity, after.identity);
    after.check(&grant).unwrap();
    let identity = &after.clocks.driver.as_ref().unwrap().identity;
    assert!(grant.bind_driver(identity).unwrap());
    assert!(!grant.bind_driver(identity).unwrap());
    assert_eq!(grant.driver.as_deref(), Some(identity.as_str()));
}

#[test]
fn unreviewed_automatic_policy_is_rejected_even_when_source_intent_is_unchanged() {
    let h = Harness::new(false);
    let mut settings = h.preview().file.settings;
    settings.gpu_power = None;
    settings.gpu = None;
    let before = approval(&h, &settings).unwrap();
    let grant = before.grant().unwrap();
    assert_eq!(grant.automatic_power_off, Some(vec![true]));
    h.environment
        .io
        .0
        .lock()
        .unwrap()
        .device
        .automatic_power_off = false;
    let after = approval(&h, &settings).unwrap();
    assert_eq!(after.identity, before.identity);
    assert!(after.check(&grant).is_err());
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn a_known_driver_range_change_cannot_replace_the_original_grant() {
    let h = Harness::new(false);
    let settings = h.preview().file.settings;
    let before = approval(&h, &settings).unwrap();
    let grant = before.grant().unwrap();
    let original = grant.driver.clone().unwrap();
    h.memory.0.lock().unwrap().offsets[0].core.max_khz -= 1000;
    let after = approval(&h, &settings).unwrap();
    assert_eq!(after.identity, before.identity);
    assert!(after.check(&grant).is_err());
    assert_eq!(grant.driver.as_deref(), Some(original.as_str()));
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn background_grant_checks_future_ac_limits_while_current_battery_target_is_valid() {
    let h = Harness::new(false);
    let mut settings = h.preview().file.settings;
    settings.oem_power_mode = Some(2);
    settings.gpu = None;
    {
        let mut data = h.memory.0.lock().unwrap();
        data.source.on_ac = Some(false);
        data.offsets[0].core = domain(0, -50_000, 50_000);
        data.offsets[0].memory.delta_khz = 0;
    }
    {
        let session = context(&h.f, &h.memory, &h.gpu, &h.power)
            .session()
            .unwrap();
        let _gate = h.f.hal.begin_write().unwrap();
        assert!(
            crate::profiles::preview_mode(&session.context, &settings)
                .unwrap()
                .is_some()
        );
    }
    let error = approval(&h, &settings).err().unwrap();
    assert!(format!("{error:#}").contains("未来供电"));
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn future_explicit_automatic_policy_cannot_authorize_an_unqualified_shutdown() {
    let h = Harness::new(false);
    let mut settings = h.preview().file.settings;
    settings.gpu = None;
    settings
        .gpu_power
        .as_mut()
        .unwrap()
        .values
        .automatic_power_off = true;
    {
        let mut data = h.environment.io.0.lock().unwrap();
        data.device.automatic_power_off = false;
        data.device.drives_display = true;
        data.device.can_disable = false;
    }
    {
        let session = context(&h.f, &h.memory, &h.gpu, &h.power)
            .session()
            .unwrap();
        let _gate = h.f.hal.begin_write().unwrap();
        assert!(
            crate::profiles::preview_mode(&session.context, &settings)
                .unwrap()
                .is_some()
        );
    }
    assert!(format!("{:#}", approval(&h, &settings).err().unwrap()).contains("未来独显"));
    assert!(h.environment.io.0.lock().unwrap().events.is_empty());
}

#[test]
fn grant_contract_requires_nullable_fields_and_rejects_core_only_legacy_targets() {
    let h = Harness::new(true);
    let preview = h.preview();
    let value = serde_json::to_value(
        approval(&h, &preview.file.settings)
            .unwrap()
            .grant()
            .unwrap(),
    )
    .unwrap();
    for field in ["automaticPowerOff", "driver", "driverDeferred"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Grant>(missing).is_err());
    }
    assert!(serde_json::from_value::<Grant>(serde_json::to_value(&preview.mode).unwrap()).is_err());
}
