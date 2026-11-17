use super::*;
use std::cell::RefCell;

struct Memory(RefCell<State>);
struct State {
    version: [u8; 2],
    ui: u8,
    style: u8,
    raw: u16,
    power: PowerStatus,
    requests: Vec<[u8; 256]>,
    echo: bool,
}
impl Memory {
    fn new() -> Self {
        Self(RefCell::new(State {
            version: [1, 0],
            ui: 1,
            style: 0,
            raw: 2832,
            power: PowerStatus {
                present: Some(true),
                ac_online: Some(false),
                percent: Some(80),
                ..PowerStatus::default()
            },
            requests: vec![],
            echo: false,
        }))
    }
    fn identity(&self) -> String {
        rule(self).unwrap().identity().unwrap()
    }
}
impl Io for Memory {
    fn query(&self, request: &[u8; 256]) -> Result<[u8; 256]> {
        let mut state = self.0.borrow_mut();
        state.requests.push(*request);
        let mut response = [0; 256];
        match request[0] {
            8 => {
                response[..2].copy_from_slice(&state.version);
                response[17] = state.ui << 4;
            }
            35 => response[0] = state.style,
            1 => {
                if request[1..7] != [12, 90, 22, 8, 0, 223] || request[7..].iter().any(|v| *v != 0)
                {
                    bail!("隔离温度请求不匹配");
                }
                if state.echo {
                    response[1..6].copy_from_slice(&request[1..6]);
                } else {
                    response[1..3].copy_from_slice(&state.raw.to_le_bytes());
                }
            }
            _ => bail!("意外的隔离命令"),
        }
        Ok(response)
    }
    fn power(&self) -> Result<PowerStatus> {
        Ok(self.0.borrow().power.clone())
    }
}

#[test]
fn capability_uses_exact_ui_and_style_rules_without_persisting_oem_settings() {
    let io = Memory::new();
    for ui in [1, 2] {
        io.0.borrow_mut().ui = ui;
        io.0.borrow_mut().requests.clear();
        assert!(rule(&io).is_ok());
        assert_eq!(
            io.0.borrow()
                .requests
                .iter()
                .map(|r| r[0])
                .collect::<Vec<_>>(),
            [8]
        );
    }
    io.0.borrow_mut().ui = 0;
    io.0.borrow_mut().style = 3;
    assert!(rule(&io).is_ok());
    io.0.borrow_mut().style = 2;
    assert!(rule(&io).is_err());
    io.0.borrow_mut().version = [2, 0];
    assert!(rule(&io).is_err());
}

#[test]
fn integer_kelvin_conversion_and_percent_boundaries_match_original() {
    let io = Memory::new();
    let identity = io.identity();
    for (raw, percent, expected) in [
        (2841, 80, Some((10, 80))),
        (2842, 80, None),
        (2832, 81, None),
        (2729, 80, Some((0, 80))),
        (2722, 80, Some((-1, 80))),
    ] {
        let mut data = io.0.borrow_mut();
        data.raw = raw;
        data.power.percent = Some(percent);
        drop(data);
        assert_eq!(inspect(&io, &identity, Trigger::Event).unwrap(), expected);
    }
}

#[test]
fn startup_and_resume_query_temperature_only_on_confirmed_battery_power() {
    let io = Memory::new();
    let identity = io.identity();
    for trigger in [Trigger::Startup, Trigger::Resume] {
        io.0.borrow_mut().power.ac_online = Some(true);
        io.0.borrow_mut().requests.clear();
        assert_eq!(inspect(&io, &identity, trigger).unwrap(), None);
        assert!(!io.0.borrow().requests.iter().any(|r| r[0] == 1));
        io.0.borrow_mut().power.ac_online = Some(false);
        assert_eq!(inspect(&io, &identity, trigger).unwrap(), Some((10, 80)));
    }
    io.0.borrow_mut().power.ac_online = None;
    assert!(inspect(&io, &identity, Trigger::Resume).is_err());
    assert_eq!(
        inspect(&io, &identity, Trigger::Event).unwrap(),
        Some((10, 80))
    );
}

#[test]
fn invalid_reports_unknown_battery_and_changed_identity_never_warn() {
    let io = Memory::new();
    let identity = io.identity();
    for raw in [0, u16::MAX] {
        io.0.borrow_mut().raw = raw;
        assert!(inspect(&io, &identity, Trigger::Event).is_err());
    }
    io.0.borrow_mut().echo = true;
    assert!(inspect(&io, &identity, Trigger::Event).is_err());
    io.0.borrow_mut().echo = false;
    io.0.borrow_mut().raw = 2832;
    io.0.borrow_mut().power.percent = None;
    assert!(inspect(&io, &identity, Trigger::Event).is_err());
    io.0.borrow_mut().power.percent = Some(101);
    assert!(inspect(&io, &identity, Trigger::Event).is_err());
    io.0.borrow_mut().power.percent = Some(80);
    io.0.borrow_mut().power.present = None;
    assert!(inspect(&io, &identity, Trigger::Event).is_err());
    io.0.borrow_mut().ui = 2;
    assert!(inspect(&io, &identity, Trigger::Event).is_err());
}
