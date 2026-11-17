use super::*;
use std::cell::RefCell;
#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Firmware(bool),
    Node(bool),
    Save(bool),
}
struct Fake {
    device: RefCell<Device>,
    firmware: RefCell<bool>,
    events: RefCell<Vec<Event>>,
    fail: Vec<usize>,
    ignore: Vec<usize>,
    partial: bool,
    unsafe_after: Option<usize>,
}
impl Fake {
    fn new(enabled: bool) -> Self {
        Self {
            device: RefCell::new(Device {
                id: "ACPI\\NVDA0820\\ISOLATED".into(),
                name: "Memory".into(),
                enabled,
                automatic_power_off: !enabled,
                problem: if enabled { 0 } else { 22 },
                drives_display: false,
                can_disable: true,
            }),
            firmware: RefCell::new(enabled),
            events: RefCell::new(Vec::new()),
            fail: Vec::new(),
            ignore: Vec::new(),
            partial: true,
            unsafe_after: None,
        }
    }
    fn plan(&self, enabled: bool) -> Plan {
        let device = self.device.borrow();
        Plan {
            id: device.id.clone(),
            expected: device.target(),
            target: Target::requested(enabled),
        }
    }
    fn event(&self, event: Event, action: impl FnOnce()) -> Result<()> {
        self.events.borrow_mut().push(event);
        let number = self.events.borrow().len();
        let failed = self.fail.contains(&number);
        if !self.ignore.contains(&number) && (!failed || self.partial) {
            action();
        }
        if failed {
            return Err(NativeCode(13).into());
        }
        Ok(())
    }
    fn state(&self) -> (Target, bool) {
        (self.device.borrow().target(), *self.firmware.borrow())
    }
}
impl Io for Fake {
    fn inspect(&self, id: &str) -> Result<Device> {
        let mut device = self.device.borrow().clone();
        if id != device.id {
            bail!("wrong instance");
        }
        if self
            .unsafe_after
            .is_some_and(|n| self.events.borrow().len() >= n)
        {
            device.can_disable = false;
            device.drives_display = true;
        }
        Ok(device)
    }
    fn firmware(&self, enabled: bool) -> Result<()> {
        self.event(Event::Firmware(enabled), || {
            *self.firmware.borrow_mut() = enabled
        })
    }
    fn node(&self, _: &str, enabled: bool) -> Result<()> {
        self.event(Event::Node(enabled), || {
            let mut device = self.device.borrow_mut();
            device.enabled = enabled;
            device.problem = if enabled { 0 } else { 22 };
        })
    }
    fn save(&self, automatic: bool) -> Result<()> {
        self.event(Event::Save(automatic), || {
            self.device.borrow_mut().automatic_power_off = automatic
        })
    }
}
#[test]
fn original_disable_and_enable_order_and_policy_are_exact() {
    let io = Fake::new(true);
    execute(&io, &io.plan(false)).unwrap();
    assert_eq!(
        *io.events.borrow(),
        vec![
            Event::Node(false),
            Event::Node(false),
            Event::Node(false),
            Event::Firmware(false),
            Event::Save(true)
        ]
    );
    assert_eq!(io.state(), (Target::requested(false), false));
    io.events.borrow_mut().clear();
    execute(&io, &io.plan(true)).unwrap();
    assert_eq!(
        *io.events.borrow(),
        vec![Event::Firmware(true), Event::Node(true), Event::Save(false)]
    );
}
#[test]
fn transient_node_failures_retry_but_never_send_firmware_off_before_disabled() {
    let mut io = Fake::new(true);
    io.fail = vec![1, 2];
    io.partial = false;
    execute(&io, &io.plan(false)).unwrap();
    assert_eq!(io.state(), (Target::requested(false), false));
    let mut io = Fake::new(true);
    io.fail = vec![1, 2, 3];
    io.partial = false;
    let error = execute(&io, &io.plan(false)).unwrap_err();
    assert_eq!(error.phase, Phase::NodeOff);
    assert_eq!(error.code, 13);
    assert!(!error.recovery_failed);
    assert!(!io.events.borrow().contains(&Event::Firmware(false)));
    assert_eq!(io.state(), (Target::requested(true), true));
}
#[test]
fn each_enable_failure_compensates_both_node_and_saved_policy() {
    for (call, phase) in [(1, Phase::FirmwareOn), (2, Phase::NodeOn), (3, Phase::Save)] {
        let mut io = Fake::new(false);
        io.fail = vec![call];
        let before = io.state();
        let error = execute(&io, &io.plan(true)).unwrap_err();
        assert_eq!(error.phase, phase);
        assert!(!error.recovery_failed);
        assert_eq!(io.state(), before);
    }
}
#[test]
fn firmware_off_and_save_failures_restore_before_on_in_original_order() {
    for (call, phase) in [(4, Phase::FirmwareOff), (5, Phase::Save)] {
        let mut io = Fake::new(true);
        io.fail = vec![call];
        let error = execute(&io, &io.plan(false)).unwrap_err();
        assert_eq!(error.phase, phase);
        assert!(!error.recovery_failed);
        assert_eq!(io.state(), (Target::requested(true), true));
        assert_eq!(
            &io.events.borrow()[call..],
            &[Event::Firmware(true), Event::Node(true), Event::Save(false)]
        );
    }
}
#[test]
fn ignored_save_is_a_readback_failure_and_compensates() {
    let mut io = Fake::new(true);
    io.ignore = vec![5];
    let error = execute(&io, &io.plan(false)).unwrap_err();
    assert_eq!(error.phase, Phase::Verify);
    assert_eq!(io.state(), (Target::requested(true), true));
}
#[test]
fn compensation_failure_is_reported_without_discarding_error_phase() {
    let mut io = Fake::new(false);
    io.fail = vec![2, 6];
    let error = execute(&io, &io.plan(true)).unwrap_err();
    assert_eq!(error.phase, Phase::NodeOn);
    assert!(error.recovery_failed);
    let text = Failure::describe(error.exit_code());
    assert!(text.contains("控制节点开启"));
    assert!(text.contains("补偿未完成"));
    assert!(text.contains("13"));
}
#[test]
fn stale_policy_fault_and_active_output_reject_before_any_operation() {
    for change in 0..3 {
        let io = Fake::new(true);
        let plan = io.plan(false);
        match change {
            0 => io.device.borrow_mut().automatic_power_off = true,
            1 => io.device.borrow_mut().problem = 43,
            _ => io.device.borrow_mut().can_disable = false,
        }
        assert_eq!(execute(&io, &plan).unwrap_err().phase, Phase::Preflight);
        assert!(io.events.borrow().is_empty());
    }
}
#[test]
fn output_change_during_disable_prevents_firmware_off_and_recovers_on() {
    let mut io = Fake::new(true);
    io.unsafe_after = Some(3);
    assert_eq!(
        execute(&io, &io.plan(false)).unwrap_err().phase,
        Phase::Preflight
    );
    assert!(!io.events.borrow().contains(&Event::Firmware(false)));
    assert_eq!(io.state(), (Target::requested(true), true));
}
#[test]
fn initial_mixed_policy_is_preserved_by_compensation() {
    let mut io = Fake::new(false);
    io.device.borrow_mut().automatic_power_off = false;
    io.fail = vec![3];
    let before = io.state();
    assert!(execute(&io, &io.plan(true)).is_err());
    assert_eq!(io.state(), before);
}
#[test]
fn unchanged_target_is_noop_even_when_display_becomes_active() {
    let io = Fake::new(false);
    io.device.borrow_mut().can_disable = false;
    execute(&io, &io.plan(false)).unwrap();
    assert!(io.events.borrow().is_empty());
}
#[test]
fn plan_roundtrip_rejects_non_oem_ambiguous_and_unknown_fields() {
    let io = Fake::new(true);
    let plan = io.plan(false);
    let decoded: Plan = serde_json::from_slice(&serde_json::to_vec(&plan).unwrap()).unwrap();
    assert_eq!(decoded.id, plan.id);
    assert_eq!(decoded.expected, plan.expected);
    assert_eq!(decoded.target, plan.target);
    for id in [
        "PCI\\VEN_10DE&DEV_1234\\4&1",
        "ACPI\\NVDA0820\\",
        "ACPI\\NVDA0820\\A\\B",
        "ACPI\\NVDA0820\\*",
        "ACPI\\NVDA0820\\A\" /reboot",
        "ACPI\\NVDA0820\\A\n",
    ] {
        let mut invalid = plan.clone();
        invalid.id = id.into();
        assert!(invalid.validate().is_err());
    }
    let mut extra = serde_json::to_value(&plan).unwrap();
    extra["old"] = true.into();
    assert!(serde_json::from_value::<Plan>(extra).is_err());
}
#[test]
fn unexpected_exit_code_is_distinct_from_transaction_failure() {
    assert!(Failure::describe(0xC0000005).contains("异常退出"));
    assert!(Failure::describe(0x43000000).contains("异常退出"));
    assert!(
        Failure::describe(Failure::new(Phase::Preflight, "stale").exit_code()).contains("前态检查")
    );
}
