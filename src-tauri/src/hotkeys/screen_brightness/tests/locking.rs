use super::*;
use std::sync::Arc;

#[test]
fn cancelling_before_any_output_does_not_start_another_panel_query_for_empty_compensation() {
    let (controller, hal, memory) = setup();
    let id = identity(&controller, &hal);
    let reads = memory.0.lock().unwrap().panel_reads;
    let result = controller
        .run(
            &hal,
            &id,
            78,
            || Ok(true),
            || {
                memory.0.lock().unwrap().fail_panel = true;
                Ok(false)
            },
        )
        .unwrap();
    assert!(result.is_none());
    assert_eq!(memory.0.lock().unwrap().panel_reads, reads + 1);
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn panel_queries_never_hold_hardware_or_domain_locks_in_read_run_or_restore() {
    let (controller, mut hal, memory) = setup();
    let controller = Arc::new(controller);
    let weak = Arc::downgrade(&controller);
    let gate = hal.test_write_gate();
    let report = memory.clone();
    hal.test_screen_brightness = Some(Arc::new(move || {
        let controller = weak.upgrade().unwrap();
        let _domain = controller
            .state
            .try_lock()
            .map_err(|_| anyhow!("领域锁仍被查询持有"))?;
        let _hardware = gate.try_lock().map_err(|_| anyhow!("硬件锁仍被查询持有"))?;
        transaction::Io::panel(report.as_ref())
    }));
    let id = identity(&controller, &hal);
    let mut waits = 0;
    controller
        .run(
            &hal,
            &id,
            78,
            || Ok(true),
            || {
                waits += 1;
                if waits == 2 {
                    memory.0.lock().unwrap().snapshot.reading.current = 40;
                }
                Ok(true)
            },
        )
        .unwrap();
    let profile = controller.read(&hal).unwrap();
    controller
        .restore(&hal, profile.restore_revision.as_deref().unwrap())
        .unwrap();
    assert_eq!(memory.0.lock().unwrap().plans[&1], [75, 65]);
}

#[test]
fn pending_low_battery_record_blocks_normal_brightness_until_it_is_cleared() {
    let (mut controller, hal, memory) = setup();
    let id = identity(&controller, &hal);
    let mut waits = 0;
    controller
        .run(
            &hal,
            &id,
            78,
            || Ok(true),
            || {
                waits += 1;
                if waits == 2 {
                    memory.0.lock().unwrap().snapshot.reading.current = 40;
                }
                Ok(true)
            },
        )
        .unwrap();
    let normal = controller.read(&hal).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("low-brightness.json");
    controller.low_store = Some(low_battery::FileStore(path.clone()));
    let baseline = serde_json::json!({ "rule": { "version": [1, 0], "customer": 0, "feature": true },
        "panel": "isolated-panel", "plan": format!("{:032x}", 1), "levels": [40, 40], "cache": [0, 0] });
    let mut current = baseline.clone();
    current["levels"] = serde_json::json!([0, 0]);
    current["cache"] = serde_json::json!([40, 20]);
    let record = serde_json::json!({ "version": 2, "phase": "applied", "baseline": baseline,
        "levelsOwned": [true, true], "cacheOwned": [true, true], "current": current, "pending": null });
    std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    let blocked = controller.read(&hal).unwrap();
    assert!(blocked.restore_revision.is_none());
    assert!(blocked.restore_error.unwrap().contains("低电量亮度"));
    let writes = memory.0.lock().unwrap().writes.len();
    assert!(
        controller
            .run(&hal, &id, 78, || Ok(true), || Ok(true))
            .is_err()
    );
    assert!(
        controller
            .restore(&hal, normal.restore_revision.as_deref().unwrap())
            .is_err()
    );
    assert_eq!(memory.0.lock().unwrap().writes.len(), writes);
    std::fs::remove_file(path).unwrap();
    controller
        .restore(&hal, normal.restore_revision.as_deref().unwrap())
        .unwrap();
    assert_eq!(memory.0.lock().unwrap().plans[&1], [75, 65]);
}
