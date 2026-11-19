use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn base_power_mode_support_is_used_when_firmware_has_no_explicit_mode_mask() {
    let io = Memory::new(3);
    io.pages.borrow_mut()[7][17] = 0;
    io.pages.borrow_mut()[7][13] = 1;
    assert_eq!(available(&io).unwrap(), [0, 1, 2, 3]);
    io.pages.borrow_mut()[7].fill(0);
    io.features.set([1, 0]);
    assert_eq!(available(&io).unwrap(), [0, 1, 2, 3]);
    io.features.set([0, 0]);
    assert!(available(&io).unwrap().is_empty());
    assert!(io.commands.borrow().is_empty());
}

fn hardware_energy() -> energy::State {
    energy::State {
        hardware_p_state: true,
        energy_star: None,
    }
}

#[test]
fn mode_targets_require_an_explicit_ac_preference_field() {
    let io = Memory::new(2);
    let value = serde_json::to_value(policy::target(&io, 3, hardware_energy()).unwrap()).unwrap();
    assert_eq!(value["acMode"], 3);
    assert!(value["energy"].is_object());
    let mut missing_energy = value.clone();
    missing_energy.as_object_mut().unwrap().remove("energy");
    assert!(serde_json::from_value::<Target>(missing_energy).is_err());
    let mut old = value.clone();
    old.as_object_mut().unwrap().remove("acMode");
    assert!(serde_json::from_value::<Target>(old).is_err());
    let mut automatic = value;
    automatic["acMode"] = serde_json::Value::Null;
    assert_eq!(
        serde_json::from_value::<Target>(automatic).unwrap().ac_mode,
        None
    );
}

struct Memory {
    features: Cell<[u32; 2]>,
    pages: RefCell<[[u8; 256]; 8]>,
    commands: RefCell<Vec<u32>>,
    fail_command: Cell<bool>,
    tables: RefCell<std::collections::BTreeMap<u8, [u8; 256]>>,
}

impl Memory {
    fn new(mode: u8) -> Self {
        let mut pages = [[0; 256]; 8];
        pages[1][1] = mode;
        pages[7][0] = 1;
        pages[7][17] = 15;
        Self {
            features: Cell::new([0, 0]),
            pages: RefCell::new(pages),
            commands: RefCell::new(vec![]),
            fail_command: Cell::new(false),
            tables: RefCell::new(Default::default()),
        }
    }
}

impl SettingsAccess for Memory {
    fn read(&self, page: u8, offset: usize, length: usize) -> Result<Vec<u8>, String> {
        Ok(self.pages.borrow()[page as usize][offset..offset + length].to_vec())
    }
    fn write(&self, page: u8, offset: usize, data: &[u8]) -> Result<(), String> {
        self.pages.borrow_mut()[page as usize][offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }
}

impl ModeIo for Memory {
    fn feature_flags(&self, command: i32) -> Result<u32> {
        Ok(self.features.get()[usize::from(command == 96)])
    }
    fn fan_info(&self) -> Result<[u8; 256]> {
        Ok([0; 256])
    }
    fn table(&self, selector: u8) -> Result<[u8; 256]> {
        Ok(self
            .tables
            .borrow()
            .get(&selector)
            .copied()
            .unwrap_or([0; 256]))
    }
    fn send_mode(&self, command: u32) -> Result<()> {
        self.commands.borrow_mut().push(command);
        if self.fail_command.replace(false) {
            bail!("模拟即时命令失败");
        }
        Ok(())
    }
}

#[test]
fn performance_encoding_and_recovery_preserve_turbo_and_dtt_flags() {
    let io = Memory::new(2);
    io.pages.borrow_mut()[1][32] = 1;
    io.pages.borrow_mut()[4][8] = 1;
    let target = policy::target(&io, 2, hardware_energy()).unwrap();
    let before = capture(&io, true).unwrap();
    assert_eq!(target.command(), 0xc2);
    io.pages.borrow_mut()[1][1] = 3;
    io.pages.borrow_mut()[1][32] = 0;
    io.pages.borrow_mut()[4][8] = 0;
    recover(&io, before).unwrap();
    assert_eq!(*io.commands.borrow(), [0xc2]);
    assert_eq!(byte(&io, 1, 1).unwrap(), 2);
    assert_eq!(byte(&io, 1, 32).unwrap(), 1);
    assert_eq!(byte(&io, 4, 8).unwrap(), 1);
}

#[test]
fn unknown_modes_and_unsupported_capabilities_never_write() {
    let io = Memory::new(0);
    for mode in [4, 255] {
        assert!(policy::target(&io, mode, hardware_energy()).is_err());
    }
    io.pages.borrow_mut()[7][17] = 0;
    assert!(policy::target(&io, 2, hardware_energy()).is_err());
    io.pages.borrow_mut()[7][17] = 15;
    io.pages.borrow_mut()[7][0] = 2;
    assert!(policy::target(&io, 2, hardware_energy()).is_err());
    assert!(io.commands.borrow().is_empty());
}

#[test]
fn unknown_initial_mode_rejects_before_write() {
    let io = Memory::new(5);
    assert!(capture(&io, true).is_err());
    assert!(io.commands.borrow().is_empty());
}

#[test]
fn recovery_failure_is_reported() {
    let io = Memory::new(1);
    io.fail_command.set(true);
    assert!(
        recover(
            &io,
            CoreSnapshot {
                mode: 1,
                command: 1,
                preferences: [0, 0],
                ac_mode: Some(0),
            }
        )
        .is_err()
    );
    assert_eq!(byte(&io, 1, 1).unwrap(), 1);
}

#[test]
fn quiet_plus_is_distinct_saved_mode_with_quiet_immediate_command() {
    let io = Memory::new(4);
    io.pages.borrow_mut()[7][17] = 0x8f;
    assert_eq!(available(&io).unwrap(), [1, 2, 3, 4]);
    let target = policy::target(&io, 4, hardware_energy()).unwrap();
    assert_eq!(target.mode, 4);
    assert_eq!(target.command(), 0);
    let before = capture(&io, true).unwrap();
    io.pages.borrow_mut()[1][1] = 2;
    recover(&io, before).unwrap();
    assert_eq!(*io.commands.borrow(), [0]);
    assert_eq!(byte(&io, 1, 1).unwrap(), 4);
}
#[test]
fn policy_uses_distinct_quiet_plus_aspm_and_power_plan_slots() {
    let io = Memory::new(3);
    {
        let mut pages = io.pages.borrow_mut();
        pages[7][17] = 0x8f;
        pages[7][16] = 8;
        pages[7][19] = 2;
        pages[7][20] = 0x40;
        pages[4][70..78].copy_from_slice(&[8, 0, 1, 5, 12, 23, 34, 45]);
    }
    let mut aspm = [0; 256];
    aspm[..9].copy_from_slice(&[15, 0, 1, 1, 2, 2, 0, 0, 2]);
    let mut plan = [0; 256];
    plan[..5].copy_from_slice(&[29, 1, 0, 2, 0]);
    io.tables.borrow_mut().insert(15, aspm);
    io.tables.borrow_mut().insert(29, plan);
    let target = policy::target(&io, 4, hardware_energy()).unwrap();
    assert_eq!(target.fan_mode, 8);
    assert_eq!(target.fan_offset_pct, 12);
    assert_eq!(target.aspm, Some([1, 2]));
    assert_eq!(target.power_plan, Some(2));
    assert_eq!(target.command(), 0);
    let perf = policy::target(&io, 2, hardware_energy()).unwrap();
    assert_eq!(perf.fan_mode, 1);
    assert_eq!(perf.fan_offset_pct, 34);
    assert_eq!(perf.aspm, Some([0, 1]));
    assert_eq!(perf.power_plan, Some(1));
    assert!(io.commands.borrow().is_empty());
}
#[test]
fn invalid_linked_policy_and_unknown_target_fields_are_rejected() {
    let io = Memory::new(3);
    io.pages.borrow_mut()[7][20] = 0x40;
    io.pages.borrow_mut()[4][70] = 255;
    assert!(policy::target(&io, 0, hardware_energy()).is_err());
    io.pages.borrow_mut()[4][70] = 0;
    io.pages.borrow_mut()[4][74] = 101;
    assert!(policy::target(&io, 0, hardware_energy()).is_err());
    io.pages.borrow_mut()[4][74] = 0;
    let target = policy::target(&io, 0, hardware_energy()).unwrap();
    let mut value = serde_json::to_value(&target).unwrap();
    value["command"] = 123.into();
    assert!(serde_json::from_value::<Target>(value).is_err());
    assert!(io.commands.borrow().is_empty());
}

#[test]
fn fixed_aspm_capability_uses_original_modes_without_requiring_an_extended_table() {
    let io = Memory::new(3);
    {
        let mut pages = io.pages.borrow_mut();
        pages[7][11] = 2;
    }
    for mode in 0..=4 {
        io.pages.borrow_mut()[7][17] = if mode == 4 { 0x8f } else { 0x0f };
        let target = policy::target(&io, mode, hardware_energy()).unwrap();
        assert_eq!(target.aspm, Some([if mode == 2 { 0 } else { 2 }; 2]));
        assert!(target.power_plan.is_none());
    }
    assert!(io.commands.borrow().is_empty());
    io.pages.borrow_mut()[7][11] = 0;
    assert!(
        policy::target(&io, 2, hardware_energy())
            .unwrap()
            .aspm
            .is_none()
    );
}

#[test]
fn extended_aspm_overrides_fixed_policy_and_invalid_values_never_use_fixed_policy() {
    let io = Memory::new(3);
    io.pages.borrow_mut()[7][11] = 2;
    io.pages.borrow_mut()[7][16] = 8;
    let mut table = [0; 256];
    table[1..3].copy_from_slice(&[1, 2]);
    io.tables.borrow_mut().insert(15, table);
    assert_eq!(
        policy::target(&io, 2, hardware_energy()).unwrap().aspm,
        Some([1, 2])
    );
    table[1] = 3;
    io.tables.borrow_mut().insert(15, table);
    assert!(policy::target(&io, 2, hardware_energy()).is_err());
    assert!(io.commands.borrow().is_empty());
}

#[test]
fn legacy_header_never_infers_new_companion_or_quiet_plus_bits() {
    let io = Memory::new(3);
    {
        let mut pages = io.pages.borrow_mut();
        pages[7][0] = 0;
        pages[7][17] = 0x8f;
        pages[7][16] = 8;
        pages[7][11] = 2;
        pages[7][19] = 2;
        pages[7][20] = 1;
        pages[4][9] = 1;
    }
    assert_eq!(available(&io).unwrap(), [0, 1, 2, 3]);
    let target = policy::target(&io, 3, hardware_energy()).unwrap();
    assert!(target.aspm.is_none());
    assert!(target.power_plan.is_none());
    assert_eq!(target.fan_mode, 0);
}
