use super::engine::{Binding, Engine, Io};
use anyhow::{Result, bail, ensure};
use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

struct Memory {
    binding: RefCell<Binding>,
    calls: RefCell<Vec<String>>,
    low: Cell<bool>,
    fail: RefCell<Option<String>>,
    available: Cell<bool>,
    replace_on: RefCell<Option<String>>,
}
impl Memory {
    fn new() -> Self {
        Self {
            binding: RefCell::new(Binding {
                brightness: "panel-plan".into(),
                customer: 0,
                low_control: true,
                pc_beep: true,
                codec: Some("codec".into()),
            }),
            calls: RefCell::new(vec![]),
            low: Cell::new(true),
            fail: RefCell::new(None),
            available: Cell::new(true),
            replace_on: RefCell::new(None),
        }
    }
    fn identity(&self) -> String {
        self.binding.borrow().identity().unwrap()
    }
    fn call(&self, value: String) -> Result<()> {
        self.calls.borrow_mut().push(value.clone());
        if self.replace_on.borrow().as_ref() == Some(&value) {
            self.binding.borrow_mut().codec = Some("replacement".into());
        }
        if self.fail.borrow().as_ref() == Some(&value) {
            bail!("隔离阶段失败：{value}");
        }
        Ok(())
    }
}
impl Io for Memory {
    fn binding(&self) -> Result<Binding> {
        Ok(self.binding.borrow().clone())
    }
    fn available(&self) -> Result<()> {
        ensure!(self.available.get(), "隔离控制者占用");
        Ok(())
    }
    fn brightness(
        &self,
        _: &Binding,
        value: u8,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        permit()?;
        self.call(format!("brightness:{value}"))
    }
    fn low_setting(&self, _: &Binding, permit: &mut impl FnMut() -> Result<()>) -> Result<()> {
        permit()?;
        self.call("low".into())
    }
    fn mute(
        &self,
        _: &Binding,
        value: bool,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        permit()?;
        self.call(format!("mute:{value}"))
    }
    fn play(&self, value: bool) -> Result<()> {
        self.call(format!("play:{value}"))
    }
    fn low_bits(&self) -> Result<bool> {
        self.call("ec551".into())?;
        Ok(self.low.get())
    }
    fn notify(&self) -> Result<()> {
        self.call("notify:1".into())
    }
}
#[test]
fn event_186_preserves_original_stage_order_without_59_notification() {
    let io = Memory::new();
    let mut engine = Engine::default();
    engine
        .event(
            &io,
            &io.identity(),
            true,
            Some(42),
            &mut Instant::now,
            &mut || Ok(()),
        )
        .unwrap();
    assert_eq!(
        *io.calls.borrow(),
        ["brightness:42", "play:true", "mute:false", "ec551", "low"]
    );
    assert!(engine.playing());
}
#[test]
fn original_stop_keeps_the_due_notification_but_policy_cancel_drops_it() {
    let now = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    engine
        .event(&io, &io.identity(), true, None, &mut || now, &mut || Ok(()))
        .unwrap();
    engine
        .event(
            &io,
            &io.identity(),
            false,
            None,
            &mut || now,
            &mut || Ok(()),
        )
        .unwrap();
    assert!(!engine.playing());
    assert!(
        engine
            .tick(&io, now + Duration::from_secs(1), &mut || Ok(()))
            .unwrap()
    );
    assert_eq!(
        io.calls
            .borrow()
            .iter()
            .filter(|s| *s == "notify:1")
            .count(),
        1
    );
    assert!(
        !engine
            .tick(&io, now + Duration::from_secs(2), &mut || Ok(()))
            .unwrap()
    );
    engine.cancel(&io).unwrap();
    let writes = io.calls.borrow().len();
    assert!(
        engine
            .event(
                &io,
                &io.identity(),
                true,
                Some(80),
                &mut || now,
                &mut || Ok(())
            )
            .is_err()
    );
    assert_eq!(io.calls.borrow().len(), writes);
}

#[test]
fn customer_five_low_bits_suppress_the_delayed_command_but_not_the_session() {
    let now = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    io.binding.borrow_mut().customer = 5;
    engine
        .event(&io, &io.identity(), true, None, &mut || now, &mut || Ok(()))
        .unwrap();
    engine
        .tick(&io, now + Duration::from_secs(1), &mut || Ok(()))
        .unwrap();
    assert!(!io.calls.borrow().iter().any(|s| s == "notify:1"));
    assert!(engine.playing());
    engine.cancel(&io).unwrap();
}
#[test]
fn absent_optional_capabilities_skip_codec_and_low_setting_but_keep_original_timer() {
    let now = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    {
        let mut binding = io.binding.borrow_mut();
        binding.pc_beep = false;
        binding.codec = None;
        binding.low_control = false;
    }
    engine
        .event(&io, &io.identity(), true, Some(0), &mut || now, &mut || {
            Ok(())
        })
        .unwrap();
    assert_eq!(*io.calls.borrow(), ["brightness:0", "play:true"]);
    engine
        .tick(&io, now + Duration::from_secs(1), &mut || Ok(()))
        .unwrap();
    assert_eq!(
        *io.calls.borrow(),
        ["brightness:0", "play:true", "ec551", "notify:1"]
    );
}
#[test]
fn stale_binding_invalid_payload_or_permission_rejection_never_outputs() {
    let now = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    assert!(
        engine
            .event(&io, "stale", true, None, &mut || now, &mut || Ok(()))
            .is_err()
    );
    assert!(
        engine
            .event(
                &io,
                &io.identity(),
                true,
                Some(127),
                &mut || now,
                &mut || Ok(())
            )
            .is_err()
    );
    assert!(
        engine
            .event(
                &io,
                &io.identity(),
                false,
                Some(50),
                &mut || now,
                &mut || Ok(())
            )
            .is_err()
    );
    assert!(
        engine
            .event(&io, &io.identity(), true, None, &mut || now, &mut || bail!(
                "撤销"
            ))
            .is_err()
    );
    assert!(io.calls.borrow().is_empty());
}
#[test]
fn replacement_between_stages_stops_before_touching_another_codec() {
    let now = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    *io.replace_on.borrow_mut() = Some("play:true".into());
    let expected = io.identity();
    assert!(
        engine
            .event(&io, &expected, true, None, &mut || now, &mut || Ok(()))
            .is_err()
    );
    engine.cancel(&io).unwrap();
    assert_eq!(*io.calls.borrow(), ["play:true", "play:false"]);
}
#[test]
fn failed_player_start_still_owns_cleanup_and_failed_stop_can_be_retried() {
    let io = Memory::new();
    let mut engine = Engine::default();
    *io.fail.borrow_mut() = Some("play:true".into());
    assert!(
        engine
            .event(
                &io,
                &io.identity(),
                true,
                None,
                &mut Instant::now,
                &mut || Ok(())
            )
            .is_err()
    );
    *io.fail.borrow_mut() = Some("play:false".into());
    assert!(engine.cancel(&io).is_err());
    assert!(engine.playing());
    *io.fail.borrow_mut() = None;
    engine.cancel(&io).unwrap();
    assert!(!engine.playing());
}
#[test]
fn delayed_failure_is_consumed_once_and_does_not_retry_every_poll() {
    let now = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    engine
        .event(&io, &io.identity(), true, None, &mut || now, &mut || Ok(()))
        .unwrap();
    *io.fail.borrow_mut() = Some("notify:1".into());
    assert!(
        engine
            .tick(&io, now + Duration::from_secs(1), &mut || Ok(()))
            .is_err()
    );
    assert!(
        !engine
            .tick(&io, now + Duration::from_secs(2), &mut || Ok(()))
            .unwrap()
    );
    assert_eq!(
        io.calls
            .borrow()
            .iter()
            .filter(|s| *s == "notify:1")
            .count(),
        1
    );
}
#[test]
fn control_owner_change_after_timer_fired_is_still_detected_while_playing() {
    let now = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    engine
        .event(&io, &io.identity(), true, None, &mut || now, &mut || Ok(()))
        .unwrap();
    engine
        .tick(&io, now + Duration::from_secs(1), &mut || Ok(()))
        .unwrap();
    io.available.set(false);
    assert!(
        engine
            .tick(&io, now + Duration::from_secs(2), &mut || Ok(()))
            .is_err()
    );
    engine.cancel(&io).unwrap();
    assert!(!engine.playing());
}

mod packets;

#[test]
fn alert_authorization_does_not_expand_the_existing_low_brightness_grant() {
    use crate::hotkeys::model::{SCOPES, Scope};
    use ccc_core::oem_events::Action;
    assert_eq!(SCOPES.len(), 30);
    assert_eq!(
        Scope::for_action(Action::BatteryAlert {
            active: true,
            brightness: None
        }),
        Scope::BatteryAlert
    );
    assert_ne!(
        Scope::BatteryAlert,
        Scope::for_action(Action::BatteryLowBrightness(59))
    );
}

#[test]
fn running_loop_keeps_checking_ownership_after_its_timer_has_fired() {
    let now = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    engine
        .event(&io, &io.identity(), true, None, &mut || now, &mut || Ok(()))
        .unwrap();
    engine
        .tick(&io, now + Duration::from_secs(1), &mut || Ok(()))
        .unwrap();
    io.available.set(false);
    assert!(
        engine
            .tick(&io, now + Duration::from_secs(2), &mut || Ok(()))
            .is_err()
    );
    engine.cancel(&io).unwrap();
    assert_eq!(io.calls.borrow().last().unwrap(), "play:false");
    assert_eq!(
        io.calls
            .borrow()
            .iter()
            .filter(|s| *s == "notify:1")
            .count(),
        1
    );
}
#[test]
fn timer_failure_is_not_retried_at_the_eighty_millisecond_poll_rate() {
    let now = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    engine
        .event(&io, &io.identity(), true, None, &mut || now, &mut || Ok(()))
        .unwrap();
    *io.fail.borrow_mut() = Some("notify:1".into());
    assert!(
        engine
            .tick(&io, now + Duration::from_secs(1), &mut || Ok(()))
            .is_err()
    );
    assert!(
        !engine
            .tick(&io, now + Duration::from_secs(2), &mut || Ok(()))
            .unwrap()
    );
    assert_eq!(
        io.calls
            .borrow()
            .iter()
            .filter(|s| *s == "notify:1")
            .count(),
        1
    );
}

#[test]
fn notification_interval_begins_after_the_player_is_started() {
    let base = Instant::now();
    let io = Memory::new();
    let mut engine = Engine::default();
    engine
        .event(
            &io,
            &io.identity(),
            true,
            Some(42),
            &mut || {
                assert_eq!(io.calls.borrow().last().unwrap(), "play:true");
                base + Duration::from_secs(2)
            },
            &mut || Ok(()),
        )
        .unwrap();
    assert!(
        !engine
            .tick(&io, base + Duration::from_secs(1), &mut || Ok(()))
            .unwrap()
    );
    assert!(
        engine
            .tick(&io, base + Duration::from_secs(3), &mut || Ok(()))
            .unwrap()
    );
}
