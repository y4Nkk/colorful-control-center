use super::fixtures::Fixture;
use crate::lighting::lightbar::{self, Kind, Snapshot, Values};
use anyhow::{Result, bail};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

pub(super) struct State {
    pub current: Snapshot,
    pub sends: Vec<[u8; 64]>,
    pub saves: usize,
    pub fail_sends: BTreeSet<usize>,
    pub fail_saves: BTreeSet<usize>,
    pub unavailable: bool,
}
pub(super) struct Memory(pub Mutex<State>);
impl Memory {
    fn new(kind: Kind) -> Self {
        let bytes = match kind {
            Kind::X170 => [176, 1, 3, 1, 255, 0, 0, 1, 1, 2, 3, 0, 4, 5, 6, 1, 7, 8, 9],
            Kind::X58 => [
                0, 2, 3, 2, 7, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
            ],
        };
        Self(Mutex::new(State {
            current: Snapshot {
                kind,
                identity: "isolated-profile-lightbar".into(),
                bytes,
            },
            sends: vec![],
            saves: 0,
            fail_sends: Default::default(),
            fail_saves: Default::default(),
            unavailable: false,
        }))
    }
    pub fn snapshot(&self) -> Snapshot {
        self.0.lock().unwrap().current.clone()
    }
    pub fn assert_restored(&self, expected: &Snapshot) {
        let state = self.0.lock().unwrap();
        assert_eq!(state.current.bytes, expected.bytes);
        let packets = expected.packets().unwrap();
        assert!(state.sends.ends_with(&packets));
    }
}
impl lightbar::Io for Memory {
    fn ensure_available(&self) -> Result<()> {
        if self.0.lock().unwrap().unavailable {
            bail!("隔离灯条控制权不可用");
        }
        Ok(())
    }
    fn snapshot(&self) -> Result<Snapshot> {
        Ok(self.snapshot())
    }
    fn send(&self, packet: &[u8; 64]) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        state.sends.push(*packet);
        let attempt = state.sends.len();
        if state.fail_sends.remove(&attempt) {
            bail!("隔离灯条第{attempt}次发送失败");
        }
        std::thread::yield_now();
        Ok(())
    }
    fn save(&self, bytes: &[u8; 19]) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        state.current.bytes = *bytes;
        state.saves += 1;
        let attempt = state.saves;
        if state.fail_saves.remove(&attempt) {
            bail!("隔离灯条第{attempt}次部分保存失败");
        }
        Ok(())
    }
}
pub(super) fn fixture(kind: Kind) -> (Fixture, Arc<Memory>) {
    let mut f = Fixture::new();
    let io = Arc::new(Memory::new(kind));
    f.light_bar = lightbar::Controller::default().with_test_io(io.clone());
    (f, io)
}
pub(super) fn target(kind: Kind) -> Values {
    let json = match kind {
        Kind::X170 => {
            r#"{"kind":"x170","settings":{"enabled":true,"brightness":2,"speed":0,"effect":"breath","color":{"r":20,"g":40,"b":60}}}"#
        }
        Kind::X58 => {
            r#"{"kind":"x58","settings":{"brightness":1,"speed":3,"effect":"scan","colorId":6}}"#
        }
    };
    serde_json::from_str(json).unwrap()
}
