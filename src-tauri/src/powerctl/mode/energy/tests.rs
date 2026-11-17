use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn energy_contract_requires_explicit_state_and_rejects_inconsistent_ownership() {
    for value in [
        serde_json::json!({"hardwarePState":true}),
        serde_json::json!({"hardwarePState":false,"energyStar":0,"obsolete":true}),
    ] {
        assert!(serde_json::from_value::<State>(value).is_err());
    }
    for value in [
        serde_json::json!({"hardwarePState":true,"energyStar":0}),
        serde_json::json!({"hardwarePState":false,"energyStar":null}),
        serde_json::json!({"hardwarePState":false,"energyStar":3}),
    ] {
        assert!(
            serde_json::from_value::<State>(value)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    for value in [
        serde_json::json!({"hardwarePState":true,"energyStar":null}),
        serde_json::json!({"hardwarePState":false,"energyStar":0}),
        serde_json::json!({"hardwarePState":false,"energyStar":1}),
        serde_json::json!({"hardwarePState":false,"energyStar":2}),
    ] {
        let state: State = serde_json::from_value(value.clone()).unwrap();
        state.validate().unwrap();
        assert_eq!(serde_json::to_value(state).unwrap(), value);
    }
}

struct Memory {
    packet: RefCell<[u8; 256]>,
    legacy: Cell<Option<u32>>,
    calls: RefCell<Vec<u8>>,
    write_without_effect: Cell<bool>,
}
impl Memory {
    fn new(value: u8) -> Self {
        let mut packet = [0; 256];
        packet[0] = 1;
        packet[2] = value << 4;
        packet[10] = 1;
        Self {
            packet: RefCell::new(packet),
            legacy: Cell::new(None),
            calls: RefCell::new(Vec::new()),
            write_without_effect: Cell::new(false),
        }
    }
}
impl Io for Memory {
    fn packet(&self) -> Result<[u8; 256]> {
        Ok(*self.packet.borrow())
    }
    fn legacy_flags(&self) -> Result<u32> {
        self.legacy.get().ok_or_else(|| anyhow!("旧能力不可读"))
    }
    fn set(&self, value: u8) -> Result<()> {
        self.calls.borrow_mut().push(value);
        if !self.write_without_effect.get() {
            let mut packet = self.packet.borrow_mut();
            packet[2] = packet[2] & !0x30 | value << 4;
        }
        Ok(())
    }
}

#[test]
fn target_modes_follow_original_and_hardware_p_state_skips_output() {
    let io = Memory::new(0);
    let state = read(&io, true).unwrap();
    assert_eq!(
        (0..5)
            .map(|mode| target(mode, state).unwrap())
            .collect::<Vec<_>>(),
        vec![Some(2), Some(1), Some(0), Some(2), Some(1)]
    );
    io.packet.borrow_mut()[9] = 0x40;
    let hardware = read(&io, false).unwrap();
    assert_eq!(target(2, hardware).unwrap(), None);
    write(&io, false, hardware, None, &mut || Ok(())).unwrap();
    assert!(io.calls.borrow().is_empty());
}

#[test]
fn only_reviewed_supported_packets_with_restorable_bits_are_accepted() {
    let io = Memory::new(3);
    assert!(read(&io, true).is_err());
    assert!(read(&Memory::new(0), false).is_err());
    io.packet.borrow_mut()[0] = 2;
    assert!(read(&io, true).is_err());
    io.packet.borrow_mut()[0] = 0;
    io.legacy.set(Some(0));
    assert!(read(&io, true).is_err());
    io.legacy.set(Some(0x4000_0000));
    assert!(read(&io, false).unwrap().hardware_p_state);
}

#[test]
fn stale_previews_and_revoked_permission_cannot_write() {
    let io = Memory::new(0);
    let before = read(&io, true).unwrap();
    assert!(write(&io, true, before, Some(2), &mut || bail!("撤销")).is_err());
    io.packet.borrow_mut()[2] = 0x10;
    assert!(write(&io, true, before, Some(2), &mut || Ok(())).is_err());
    assert!(io.calls.borrow().is_empty());
}

#[test]
fn confirmed_write_requires_readback_and_exact_restoration() {
    let io = Memory::new(0);
    let before = read(&io, true).unwrap();
    write(&io, true, before, Some(2), &mut || Ok(())).unwrap();
    let after = read(&io, true).unwrap();
    assert_eq!(after.energy_star, Some(2));
    write(&io, true, after, before.energy_star, &mut || Ok(())).unwrap();
    assert_eq!(read(&io, true).unwrap(), before);
    io.write_without_effect.set(true);
    assert!(write(&io, true, before, Some(1), &mut || Ok(())).is_err());
    assert_eq!(*io.calls.borrow(), [2, 0, 1]);
}
