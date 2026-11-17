use super::{
    facts_tests,
    fields::{self, FileStore},
    state,
};
use anyhow::{Result, bail};

fn setup() -> (tempfile::TempDir, FileStore, facts_tests::Memory) {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore(dir.path().join("refresh.json"));
    let io = facts_tests::setup();
    io.0.borrow_mut().0.fields = [0, 0, 144];
    (dir, store, io)
}
fn restore_fields(io: &facts_tests::Memory, store: &FileStore) {
    let mut record = store.load().unwrap().unwrap();
    fields::set_active(store, &mut record, false).unwrap();
    fields::restore(io, store, &mut record, &mut || Ok(())).unwrap();
}

#[test]
fn enabling_is_explicit_and_restoring_recovers_each_original_field() {
    let (_dir, store, io) = setup();
    let before = state::read(&io, &store).unwrap();
    assert!(before.can_start && !before.active && before.initial.is_none());
    state::start(
        &io,
        &store,
        &before.revision,
        &mut || Ok(()),
        &mut || Ok(()),
    )
    .unwrap();
    let active = state::read(&io, &store).unwrap();
    assert!(active.active && active.can_restore && !active.can_start);
    assert_eq!(active.initial, Some([0, 0, 144]));
    assert_eq!(io.0.borrow().0.fields, [1, 1, 144]);
    let record = store.load().unwrap().unwrap();
    fields::verify(&io, &store, &record, &mut || Ok(())).unwrap();
    restore_fields(&io, &store);
    assert_eq!(io.0.borrow().0.fields, [0, 0, 144]);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn stale_preview_or_unknown_supply_never_starts_a_transaction() {
    let (_dir, store, io) = setup();
    let before = state::read(&io, &store).unwrap();
    io.0.borrow_mut().0.on_ac = Some(true);
    assert!(
        state::start(
            &io,
            &store,
            &before.revision,
            &mut || Ok(()),
            &mut || Ok(())
        )
        .is_err()
    );
    io.0.borrow_mut().0.on_ac = None;
    assert!(!state::read(&io, &store).unwrap().can_start);
    assert!(store.load().unwrap().is_none());
    assert_eq!(io.0.borrow().0.fields, [0, 0, 144]);
}

#[test]
fn permission_loss_after_one_field_restores_the_original_values() {
    let (_dir, store, io) = setup();
    let before = state::read(&io, &store).unwrap();
    let mut calls = 0;
    let mut permit = || -> Result<()> {
        calls += 1;
        if calls >= 4 {
            bail!("许可已撤销");
        }
        Ok(())
    };
    assert!(state::start(&io, &store, &before.revision, &mut permit, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().0.fields, [0, 0, 144]);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn external_changes_revoke_active_policy_and_do_not_get_overwritten() {
    let (_dir, store, io) = setup();
    let before = state::read(&io, &store).unwrap();
    state::start(
        &io,
        &store,
        &before.revision,
        &mut || Ok(()),
        &mut || Ok(()),
    )
    .unwrap();
    io.0.borrow_mut().0.fields[1] = 0;
    let changed = state::read(&io, &store).unwrap();
    assert!(!changed.active && changed.error.is_some());
    let record = store.load().unwrap().unwrap();
    assert!(fields::verify(&io, &store, &record, &mut || Ok(())).is_err());
    restore_fields(&io, &store);
    assert_eq!(io.0.borrow().0.fields, [0, 0, 144]);
}
