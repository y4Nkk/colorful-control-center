use super::facts::{self, Evidence, Source};
use crate::display::{Display, Mode};
use anyhow::Result;
use std::cell::RefCell;

pub(super) struct Memory(pub RefCell<(Evidence, Vec<Display>)>);
impl Source for Memory {
    fn evidence(&self) -> Result<Evidence> {
        Ok(self.0.borrow().0.clone())
    }
    fn displays(&self) -> Result<Vec<Display>> {
        Ok(self.0.borrow().1.clone())
    }
}
pub(super) fn setup() -> Memory {
    let mut packet = [0; 256];
    packet[0] = 1;
    Memory(RefCell::new((
        Evidence {
            packet,
            customer: 1,
            dc_mode_power: false,
            product: 20600,
            mode: 1,
            modes: vec![0, 1, 2, 3],
            fields: [1, 1, 144],
            on_ac: Some(false),
        },
        vec![Display {
            current: Mode {
                device: "DISPLAY1".into(),
                identity: "panel-a".into(),
                name: "Panel".into(),
                width: 1920,
                height: 1080,
                bits: 32,
                orientation: 0,
                x: 0,
                y: 0,
                flags: 0,
                frequency: 144,
            },
            frequencies: vec![144, 60, 48, 144],
            primary: true,
        }],
    )))
}
impl super::fields::Io for Memory {
    fn identity(&self) -> Result<String> {
        Ok(facts::read(self)?.identity)
    }
    fn fields(&self) -> Result<[u8; 3]> {
        Ok(self.0.borrow().0.fields)
    }
    fn write(&self, index: usize, value: u8) -> Result<()> {
        self.0.borrow_mut().0.fields[index] = value;
        Ok(())
    }
}

#[test]
fn preview_uses_original_filtered_candidates_and_saved_rate() {
    let io = setup();
    let view = facts::read(&io).unwrap();
    assert_eq!(view.candidates, [60, 144]);
    assert_eq!((view.low, view.saved_or_highest), (60, 144));
    assert_eq!(view.fields, [1, 1, 144]);
    io.0.borrow_mut().0.fields[2] = 70;
    assert_eq!(facts::read(&io).unwrap().saved_or_highest, 144);
}

#[test]
fn identity_is_stable_for_mode_supply_and_current_frequency_but_not_capability() {
    let io = setup();
    let identity = facts::read(&io).unwrap().identity;
    {
        let mut state = io.0.borrow_mut();
        state.0.mode = 2;
        state.0.on_ac = Some(true);
        state.0.fields = [1, 0, 60];
        state.1[0].current.frequency = 60;
        state.1[0].current.name = "new driver label".into();
    }
    assert_eq!(facts::read(&io).unwrap().identity, identity);
    io.0.borrow_mut().1[0].current.identity = "other".into();
    assert_ne!(facts::read(&io).unwrap().identity, identity);
}

#[test]
fn unknown_or_disabled_firmware_and_ambiguous_displays_are_rejected() {
    let io = setup();
    io.0.borrow_mut().0.product = 4106;
    assert!(facts::read(&io).is_err());
    io.0.borrow_mut().0.product = 0;
    assert!(facts::read(&io).is_err());
    io.0.borrow_mut().0.product = 20600;
    io.0.borrow_mut().0.packet[18] = 0x20;
    assert!(facts::read(&io).is_err());
    io.0.borrow_mut().0.packet[18] = 0;
    io.0.borrow_mut().0.packet[0] = 2;
    assert!(facts::read(&io).is_err());
    io.0.borrow_mut().0.packet[0] = 1;
    let extra = io.0.borrow().1[0].clone();
    io.0.borrow_mut().1.push(extra);
    assert!(facts::read(&io).is_err());
}

#[test]
fn invalid_flags_mode_and_insufficient_frequencies_never_create_preview() {
    let io = setup();
    io.0.borrow_mut().0.fields[1] = 2;
    assert!(facts::read(&io).is_err());
    io.0.borrow_mut().0.fields[1] = 1;
    io.0.borrow_mut().0.mode = 4;
    assert!(facts::read(&io).is_err());
    io.0.borrow_mut().0.mode = 1;
    io.0.borrow_mut().1[0].frequencies = vec![48, 60, 60];
    assert!(facts::read(&io).is_err());
}
