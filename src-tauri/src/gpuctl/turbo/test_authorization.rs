use super::*;

#[test]
fn repeated_event_with_matching_driver_state_does_not_write_or_replace_first_baseline() {
    let memory = fixture();
    let mut state = State::default();
    let mut shared = None;
    execute(&mut state, &memory, &mut shared, 97).unwrap();
    let initial = state.initial.clone();
    let writes = memory.0.lock().unwrap().writes.len();
    assert!(
        execute(&mut state, &memory, &mut shared, 97)
            .unwrap()
            .contains("无需修改")
    );
    assert_eq!(memory.0.lock().unwrap().writes.len(), writes);
    assert_eq!(state.initial, initial);
}

#[test]
fn identity_binds_targets_and_device_capabilities_but_allows_mode_changes() {
    let memory = fixture();
    let original = identity(&memory);
    memory.0.lock().unwrap().source.mode = 3;
    memory.0.lock().unwrap().source.on_ac = Some(false);
    assert_eq!(identity(&memory), original);
    memory.0.lock().unwrap().source.saved[0].core_mhz += 1;
    assert_ne!(identity(&memory), original);
    let expected = identity(&memory);
    memory.0.lock().unwrap().offsets[0].core.max_khz -= 1;
    assert_ne!(identity(&memory), expected);
}
