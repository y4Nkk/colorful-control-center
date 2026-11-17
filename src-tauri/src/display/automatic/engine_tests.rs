use super::{
    engine,
    facts::{self, Evidence, Source},
    fields::{FileStore, Io},
    state,
};
use crate::display::{self, Access, Display, Mode, guardian};
use anyhow::{Result, bail};
use ccc_core::oem_refresh::Trigger;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Memory(Arc<Mutex<Data>>);
struct Data {
    evidence: Evidence,
    displays: Vec<Display>,
    display_writes: usize,
    field_writes: usize,
    fail_display: Vec<usize>,
}
impl Source for Memory {
    fn evidence(&self) -> Result<Evidence> {
        Ok(self.0.lock().unwrap().evidence.clone())
    }
    fn displays(&self) -> Result<Vec<Display>> {
        self.list()
    }
}
impl Io for Memory {
    fn identity(&self) -> Result<String> {
        Ok(facts::read(self)?.identity)
    }
    fn fields(&self) -> Result<[u8; 3]> {
        Ok(self.0.lock().unwrap().evidence.fields)
    }
    fn write(&self, index: usize, value: u8) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.field_writes += 1;
        data.evidence.fields[index] = value;
        Ok(())
    }
}
impl Access for Memory {
    fn list(&self) -> Result<Vec<Display>> {
        Ok(self.0.lock().unwrap().displays.clone())
    }
    fn test(&self, _: &Mode) -> Result<()> {
        Ok(())
    }
    fn set(&self, mode: &Mode) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.display_writes += 1;
        data.displays[0].current = mode.clone();
        if data.fail_display.contains(&data.display_writes) {
            bail!("模拟显示写后故障");
        }
        Ok(())
    }
}
fn setup() -> (tempfile::TempDir, FileStore, Memory, display::Controller) {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore(dir.path().join("automatic.json"));
    let source = super::facts_tests::setup();
    let mut evidence = source.0.borrow().0.clone();
    evidence.fields = [1, 1, 100];
    let io = Memory(Arc::new(Mutex::new(Data {
        evidence,
        displays: source.0.borrow().1.clone(),
        display_writes: 0,
        field_writes: 0,
        fail_display: vec![],
    })));
    let display = display::Controller::with_guards(io.clone(), guardian::MemoryFactory);
    let revision = state::read(&io, &store).unwrap().revision;
    state::start(&io, &store, &revision, &mut || Ok(()), &mut || Ok(())).unwrap();
    (dir, store, io, display)
}
fn start(io: &Memory, store: &FileStore, display: &display::Controller) -> u64 {
    engine::execute(
        io,
        store,
        display,
        Trigger::Startup,
        None,
        &mut || Ok(()),
        &mut || Ok(()),
    )
    .unwrap()
    .unwrap()
}

#[test]
fn kept_transition_is_recorded_and_stop_restores_original_screen_and_cache() {
    let (_dir, store, io, display) = setup();
    let id = start(&io, &store, &display);
    assert_eq!(io.fields().unwrap(), [1, 1, 144]);
    let target = display.primary().unwrap();
    assert_eq!(target.frequency, 60);
    engine::keep(&io, &store, &display, id, &target, &mut || Ok(())).unwrap();
    assert!(engine::settle(&io, &store, &display, id, &mut || Ok(())).unwrap());
    let record = store.load().unwrap().unwrap();
    assert_eq!(record.owned_display().unwrap().frequency, 60);
    assert!(record.transition().is_none());
    let revision = state::read(&io, &store).unwrap().revision;
    engine::stop_and_restore(&io, &store, &display, &revision, &mut || Ok(())).unwrap();
    assert_eq!(display.primary().unwrap().frequency, 144);
    assert_eq!(io.fields().unwrap(), [1, 1, 100]);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn cancellation_rolls_back_only_this_cache_change_and_keeps_policy_active() {
    let (_dir, store, io, display) = setup();
    let id = start(&io, &store, &display);
    display.cancel(id).unwrap();
    assert!(engine::settle(&io, &store, &display, id, &mut || Ok(())).unwrap());
    assert_eq!(display.primary().unwrap().frequency, 144);
    assert_eq!(io.fields().unwrap(), [1, 1, 100]);
    assert!(state::read(&io, &store).unwrap().active);
}

#[test]
fn failed_settlement_keeps_the_outcome_until_external_cache_is_repaired() {
    let (_dir, store, io, display) = setup();
    let id = start(&io, &store, &display);
    display.cancel(id).unwrap();
    io.0.lock().unwrap().evidence.fields[2] = 120;
    assert!(engine::settle(&io, &store, &display, id, &mut || Ok(())).is_err());
    assert_eq!(io.fields().unwrap()[2], 120);
    assert!(display.automatic_outcome(id).unwrap().is_some());
    io.0.lock().unwrap().evidence.fields[2] = 144;
    assert!(engine::settle(&io, &store, &display, id, &mut || Ok(())).unwrap());
    assert_eq!(io.fields().unwrap()[2], 100);
}

#[test]
fn changed_supply_or_mode_prevents_keeping_an_outdated_trial() {
    let (_dir, store, io, display) = setup();
    let id = start(&io, &store, &display);
    let target = display.primary().unwrap();
    io.0.lock().unwrap().evidence.mode = 2;
    assert!(engine::keep(&io, &store, &display, id, &target, &mut || Ok(())).is_err());
    io.0.lock().unwrap().evidence.mode = 1;
    io.0.lock().unwrap().evidence.on_ac = Some(true);
    assert!(engine::keep(&io, &store, &display, id, &target, &mut || Ok(())).is_err());
    display.cancel(id).unwrap();
    engine::settle(&io, &store, &display, id, &mut || Ok(())).unwrap();
    assert_eq!(display.primary().unwrap().frequency, 144);
}

#[test]
fn original_amd_models_ignore_supply_events_without_losing_other_triggers() {
    let (_dir, store, io, display) = setup();
    {
        let mut data = io.0.lock().unwrap();
        data.evidence.product = 20720;
    }
    // The supported product is part of the preview identity, so recreate the policy.
    let old = store.load().unwrap().unwrap();
    std::fs::remove_file(&store.0).unwrap();
    let view = state::read(&io, &store).unwrap();
    state::start(&io, &store, &view.revision, &mut || Ok(()), &mut || Ok(())).unwrap();
    assert_ne!(old.identity(), store.load().unwrap().unwrap().identity());
    assert!(
        engine::execute(
            &io,
            &store,
            &display,
            Trigger::SupplyChange,
            None,
            &mut || Ok(()),
            &mut || Ok(())
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(start(&io, &store, &display), 1);
}

#[test]
fn interrupted_transition_can_restore_after_process_restart() {
    let (_dir, store, io, display) = setup();
    start(&io, &store, &display);
    let restarted = display::Controller::with_guards(io.clone(), guardian::MemoryFactory);
    let view = state::read(&io, &store).unwrap();
    assert!(view.error.is_some() && view.can_restore && !view.active);
    engine::stop_and_restore(&io, &store, &restarted, &view.revision, &mut || Ok(())).unwrap();
    assert_eq!(restarted.primary().unwrap().frequency, 144);
    assert_eq!(io.fields().unwrap(), [1, 1, 100]);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn restoration_never_overwrites_an_external_screen_mode_or_new_panel() {
    let (_dir, store, io, display) = setup();
    let id = start(&io, &store, &display);
    let target = display.primary().unwrap();
    engine::keep(&io, &store, &display, id, &target, &mut || Ok(())).unwrap();
    engine::settle(&io, &store, &display, id, &mut || Ok(())).unwrap();
    io.0.lock().unwrap().displays[0].current.frequency = 120;
    let view = state::read(&io, &store).unwrap();
    assert!(view.error.is_some());
    assert!(
        engine::stop_and_restore(&io, &store, &display, &view.revision, &mut || Ok(())).is_err()
    );
    assert_eq!(display.primary().unwrap().frequency, 120);
    io.0.lock().unwrap().displays[0].current.identity = "replacement".into();
    assert!(!state::read(&io, &store).unwrap().can_restore);
    assert!(store.load().unwrap().is_some());
}

#[test]
fn failed_display_recovery_keeps_intent_and_is_retryable() {
    let (_dir, store, io, display) = setup();
    io.0.lock().unwrap().fail_display = vec![1, 2];
    assert!(
        engine::execute(
            &io,
            &store,
            &display,
            Trigger::Startup,
            None,
            &mut || Ok(()),
            &mut || Ok(())
        )
        .is_err()
    );
    assert!(store.load().unwrap().unwrap().transition().is_some());
    let id = display.automatic_trial_id().unwrap().unwrap();
    io.0.lock().unwrap().fail_display.clear();
    display.cancel(id).unwrap();
    engine::settle(&io, &store, &display, id, &mut || Ok(())).unwrap();
    assert_eq!(io.fields().unwrap()[2], 100);
    assert_eq!(display.primary().unwrap().frequency, 144);
}
