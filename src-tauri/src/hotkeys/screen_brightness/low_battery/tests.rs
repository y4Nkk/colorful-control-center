use super::*;
use crate::battery::PowerStatus;
use anyhow::{Result, bail};
use std::{
    cell::RefCell,
    sync::atomic::{AtomicU64, Ordering},
};

struct Data {
    rule: model::Rule,
    known: bool,
    panel: String,
    active: u128,
    levels: [u32; 2],
    cache: [u8; 2],
    power: PowerStatus,
    writable: bool,
    writes: usize,
    failures: Vec<usize>,
    signals: Vec<u32>,
}
struct Memory(RefCell<Data>);
impl Memory {
    fn new() -> Self {
        Self(RefCell::new(Data {
            rule: model::Rule {
                version: [1, 0],
                customer: 0,
                feature: true,
            },
            known: true,
            panel: "isolated-panel".into(),
            active: 1,
            levels: [75, 45],
            cache: [0, 23],
            power: PowerStatus {
                present: Some(true),
                ac_online: Some(false),
                percent: Some(20),
                ..PowerStatus::default()
            },
            writable: true,
            writes: 0,
            failures: vec![],
            signals: vec![],
        }))
    }
    fn write(&self, f: impl FnOnce(&mut Data)) -> Result<()> {
        let mut data = self.0.borrow_mut();
        data.writes += 1;
        f(&mut data);
        if data.failures.contains(&data.writes) {
            bail!("隔离传输写入后失败");
        }
        Ok(())
    }
}
impl Io for Memory {
    fn known_firmware(&self) -> bool {
        self.0.borrow().known
    }
    fn rule(&self) -> Result<model::Rule> {
        Ok(self.0.borrow().rule.clone())
    }
    fn panel(&self) -> Result<String> {
        Ok(self.0.borrow().panel.clone())
    }
    fn active(&self) -> Result<u128> {
        Ok(self.0.borrow().active)
    }
    fn levels(&self, _: u128) -> Result<[u32; 2]> {
        Ok(self.0.borrow().levels)
    }
    fn write_level(&self, _: u128, ac: bool, value: u32) -> Result<()> {
        self.write(|data| data.levels[usize::from(!ac)] = value)
    }
    fn cache(&self) -> Result<[u8; 2]> {
        Ok(self.0.borrow().cache)
    }
    fn write_cache(&self, index: usize, value: u8) -> Result<()> {
        self.write(|data| data.cache[index] = value)
    }
    fn power(&self) -> Result<PowerStatus> {
        Ok(self.0.borrow().power.clone())
    }
    fn signal(&self, value: u32) -> Result<()> {
        self.0.borrow_mut().signals.push(value);
        Ok(())
    }
    fn activate(&self, _: u128) -> Result<()> {
        Ok(())
    }
    fn ensure_writable(&self) -> Result<()> {
        if !self.0.borrow().writable {
            bail!("隔离校准占用");
        }
        Ok(())
    }
}
struct Fixture(FileStore);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ccc-low-brightness-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self(FileStore(path))
    }
    fn identity(&self, io: &Memory) -> String {
        preview(io, &self.0).identity.unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0.0);
    }
}

#[test]
fn applies_original_setting_and_manual_restore_keeps_distinct_supply_values() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = fixture.identity(&io);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().levels, [0, 0]);
    assert_eq!(io.0.borrow().cache, [75, 20]);
    assert_eq!(fixture.identity(&io), identity);
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 45]);
    assert_eq!(io.0.borrow().cache, [0, 23]);
    assert_eq!(io.0.borrow().signals, [2]);
    assert!(fixture.0.load().unwrap().is_none());
}

#[test]
fn validates_real_capability_and_preserves_actual_preexisting_vendor_cache() {
    let io = Memory::new();
    let fixture = Fixture::new();
    io.0.borrow_mut().rule.feature = false;
    assert!(preview(&io, &fixture.0).identity.is_none());
    io.0.borrow_mut().rule.customer = 5;
    assert!(preview(&io, &fixture.0).identity.is_some());
    io.0.borrow_mut().rule.version = [0, 0];
    assert!(preview(&io, &fixture.0).identity.is_some());
    io.0.borrow_mut().rule.version = [2, 0];
    assert!(preview(&io, &fixture.0).identity.is_none());
    io.0.borrow_mut().rule.version = [1, 0];
    io.0.borrow_mut().known = false;
    assert!(preview(&io, &fixture.0).identity.is_none());
    io.0.borrow_mut().known = true;
    io.0.borrow_mut().cache[0] = 70;
    assert!(preview(&io, &fixture.0).identity.is_some());
    assert_eq!(io.0.borrow().writes, 0);
}

#[test]
fn event_on_ac_does_not_signal_or_write_and_zero_baseline_remains_recoverable() {
    let io = Memory::new();
    let fixture = Fixture::new();
    io.0.borrow_mut().levels = [0, 44];
    let identity = fixture.identity(&io);
    io.0.borrow_mut().power.ac_online = Some(true);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    assert!(io.0.borrow().signals.is_empty());
    io.0.borrow_mut().power.ac_online = Some(false);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().cache, [0, 20]);
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [0, 44]);
    assert_eq!(io.0.borrow().cache, [0, 23]);
}

#[test]
fn startup_resume_or_ac_check_restores_only_after_confirmed_recovery() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = fixture.identity(&io);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    assert!(
        check(&io, &fixture.0, &identity, &mut || Ok(()))
            .unwrap()
            .is_none()
    );
    io.0.borrow_mut().power.percent = Some(21);
    assert!(
        check(&io, &fixture.0, &identity, &mut || Ok(()))
            .unwrap()
            .is_some()
    );
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    io.0.borrow_mut().power.ac_online = None;
    assert!(check(&io, &fixture.0, &identity, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().levels, [0, 0]);
    io.0.borrow_mut().power.ac_online = Some(true);
    assert!(
        check(&io, &fixture.0, &identity, &mut || Ok(()))
            .unwrap()
            .is_some()
    );
    assert_eq!(io.0.borrow().levels, [75, 75]);
}

#[test]
fn confirmed_ac_restores_even_if_the_battery_is_no_longer_present() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = fixture.identity(&io);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    {
        let mut data = io.0.borrow_mut();
        data.power.ac_online = Some(true);
        data.power.present = Some(false);
        data.power.percent = None;
    }
    assert_eq!(
        preview(&io, &fixture.0).identity.as_deref(),
        Some(identity.as_str())
    );
    assert!(
        check(&io, &fixture.0, &identity, &mut || Ok(()))
            .unwrap()
            .is_some()
    );
    assert_eq!(io.0.borrow().levels, [75, 75]);
}

#[test]
fn repeated_event_updates_percent_without_losing_first_baseline() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = fixture.identity(&io);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    io.0.borrow_mut().power.percent = Some(15);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().cache, [75, 15]);
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 45]);
    assert_eq!(io.0.borrow().cache, [0, 23]);
}

#[test]
fn interrupted_repeated_cache_update_accepts_old_or_attempted_value_for_recovery() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = fixture.identity(&io);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    io.0.borrow_mut().power.percent = Some(15);
    let next = io.0.borrow().writes + 1;
    io.0.borrow_mut().failures = vec![next, next + 1];
    assert!(event(&io, &fixture.0, &identity, 59, &mut || Ok(())).is_err());
    assert_eq!(
        fixture
            .0
            .load()
            .unwrap()
            .unwrap()
            .pending
            .unwrap()
            .target
            .cache[1],
        15
    );
    io.0.borrow_mut().failures.clear();
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    restore(&io, &fixture.0, &revision).unwrap();
    assert_eq!(io.0.borrow().levels, [75, 45]);
    assert_eq!(io.0.borrow().cache, [0, 23]);
}

#[test]
fn partial_output_is_compensated_or_retained_for_retry() {
    for failures in [vec![1], vec![2, 3]] {
        let io = Memory::new();
        let fixture = Fixture::new();
        let identity = fixture.identity(&io);
        io.0.borrow_mut().failures = failures;
        assert!(event(&io, &fixture.0, &identity, 59, &mut || Ok(())).is_err());
        io.0.borrow_mut().failures.clear();
        if let Some(revision) = preview(&io, &fixture.0).restore_revision {
            restore(&io, &fixture.0, &revision).unwrap();
        }
        assert_eq!(io.0.borrow().levels, [75, 45]);
        assert_eq!(io.0.borrow().cache, [0, 23]);
        assert!(fixture.0.load().unwrap().is_none());
    }
}

#[test]
fn external_changes_and_stale_revisions_never_overwrite_foreign_values() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = fixture.identity(&io);
    event(&io, &fixture.0, &identity, 59, &mut || Ok(())).unwrap();
    let revision = preview(&io, &fixture.0).restore_revision.unwrap();
    io.0.borrow_mut().levels[0] = 50;
    let writes = io.0.borrow().writes;
    assert!(restore(&io, &fixture.0, &revision).is_err());
    assert_eq!(io.0.borrow().writes, writes);
    assert!(fixture.0.load().unwrap().is_some());
    io.0.borrow_mut().levels[0] = 0;
    io.0.borrow_mut().active = 2;
    assert!(preview(&io, &fixture.0).restore_revision.is_none());
    io.0.borrow_mut().active = 1;
    restore(&io, &fixture.0, &revision).unwrap();
}

#[test]
fn revocation_and_calibration_leave_no_output() {
    let io = Memory::new();
    let fixture = Fixture::new();
    let identity = fixture.identity(&io);
    io.0.borrow_mut().writable = false;
    assert!(event(&io, &fixture.0, &identity, 59, &mut || Ok(())).is_err());
    io.0.borrow_mut().writable = true;
    assert!(event(&io, &fixture.0, &identity, 59, &mut || bail!("许可撤销")).is_err());
    assert!(fixture.0.load().unwrap().is_none());
    assert_eq!(io.0.borrow().writes, 0);
    assert!(io.0.borrow().signals.is_empty());
}

#[test]
fn ac_connection_or_percent_change_between_fields_compensates_the_attempt() {
    for on_ac in [true, false] {
        let io = Memory::new();
        let fixture = Fixture::new();
        let identity = fixture.identity(&io);
        let result = event(&io, &fixture.0, &identity, 59, &mut || {
            let mut data = io.0.borrow_mut();
            if data.writes == 1 {
                if on_ac {
                    data.power.ac_online = Some(true);
                } else {
                    data.power.percent = Some(21);
                }
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(io.0.borrow().levels, [75, 45]);
        assert_eq!(io.0.borrow().cache, [0, 23]);
        assert!(fixture.0.load().unwrap().is_none());
    }
}

mod ending_tests;
