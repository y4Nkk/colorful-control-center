use super::*;
use std::cell::{Cell, RefCell};

fn points() -> [RpmPoint; LEVELS] {
    std::array::from_fn(|i| RpmPoint {
        temp_c: [35, 40, 45, 50, 60, 65, 70, 80, 90, 100][i],
        rpm_pct: [20, 30, 40, 45, 50, 60, 70, 80, 90, 100][i],
    })
}

struct Memory {
    pages: RefCell<[[u8; 256]; 8]>,
    info: [u8; 256],
    pairs: RefCell<Vec<Pair>>,
    writes: RefCell<Vec<u8>>,
    modes: RefCell<Vec<u8>>,
    corrupt_once: Cell<bool>,
    fail_once: Cell<Option<u8>>,
    fail_after: Cell<usize>,
    fail_save: Cell<bool>,
}

impl Memory {
    fn new() -> Self {
        let mut pages = [[0; 256]; 8];
        pages[7][0] = 1;
        pages[7][20] = 0x10;
        pages[1][1] = 2;
        let mut info = [0; 256];
        info[12] = 4;
        info[50] = 15;
        let pairs = [40, 42]
            .into_iter()
            .map(|query| Pair {
                query,
                tables: std::array::from_fn(|slot| {
                    let mut raw = [0x32; 112];
                    raw[10..12].copy_from_slice(
                        &(5000u16 + (query as u16 - 40) * 500 + slot as u16 * 1000).to_be_bytes(),
                    );
                    rpm::encode_curve(&raw, usize::from(query - 40) + slot, 0, &points()).unwrap()
                }),
            })
            .collect();
        Self {
            pages: RefCell::new(pages),
            info,
            pairs: RefCell::new(pairs),
            writes: RefCell::new(vec![]),
            modes: RefCell::new(vec![]),
            corrupt_once: Cell::new(false),
            fail_once: Cell::new(None),
            fail_after: Cell::new(usize::MAX),
            fail_save: Cell::new(false),
        }
    }
}

impl SettingsAccess for Memory {
    fn read(&self, page: u8, offset: usize, length: usize) -> Result<Vec<u8>, String> {
        Ok(self.pages.borrow()[page as usize][offset..offset + length].to_vec())
    }
    fn write(&self, page: u8, offset: usize, data: &[u8]) -> Result<(), String> {
        if self.fail_save.replace(false) {
            return Err("模拟保存失败".into());
        }
        self.pages.borrow_mut()[page as usize][offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }
}

impl RpmIo for Memory {
    fn fan_info(&self) -> Result<[u8; 256]> {
        Ok(self.info)
    }
    fn mode(&self, mode: u8) -> Result<()> {
        self.modes.borrow_mut().push(mode);
        Ok(())
    }
    fn exchange(&self, packet: &[u8; 256]) -> Result<[u8; 256]> {
        let write = [41, 43].contains(&packet[0]);
        let query = if write { packet[0] - 1 } else { packet[0] };
        let mut pairs = self.pairs.borrow_mut();
        let pair = pairs
            .iter_mut()
            .find(|pair| pair.query == query)
            .ok_or_else(|| anyhow!("未知表"))?;
        if write {
            self.writes.borrow_mut().push(packet[0]);
            if self.writes.borrow().len() > self.fail_after.get() {
                bail!("模拟恢复传输失败");
            }
            pair.tables[0].copy_from_slice(&packet[16..128]);
            pair.tables[1].copy_from_slice(&packet[128..240]);
            if self.corrupt_once.replace(false) {
                pair.tables[0][70] ^= 1;
            }
            if self.fail_once.get() == Some(packet[0]) {
                self.fail_once.set(None);
                bail!("模拟部分写入后失败");
            }
        }
        let mut response = [0; 256];
        response[..112].copy_from_slice(&pair.tables[0]);
        response[128..240].copy_from_slice(&pair.tables[1]);
        Ok(response)
    }
}

#[test]
fn capabilities_and_bad_tables_are_rejected_without_writes() {
    let io = Memory::new();
    io.pages.borrow_mut()[7][20] = 0;
    assert!(capture(&io).is_err());
    assert!(io.writes.borrow().is_empty());
    io.pages.borrow_mut()[7][20] = 0x10;
    io.pairs.borrow_mut()[0].tables[0][10..12].fill(0);
    assert!(capture(&io).is_err());
    assert!(io.writes.borrow().is_empty());
}

#[test]
fn rejects_missing_fan_and_invalid_curve_before_entering_custom_mode() {
    let mut io = Memory::new();
    io.info[50] = 3;
    io.info[12] = 2;
    let before = capture(&io).unwrap();
    assert!(prepare(&before, &BTreeMap::from([("sys".into(), points())])).is_err());
    assert!(prepare(&before, &BTreeMap::from([("unknown".into(), points())])).is_err());
    let mut bad = points();
    bad[3].rpm_pct = 0;
    assert!(prepare(&before, &BTreeMap::from([("cpu".into(), bad)])).is_err());
    assert!(io.modes.borrow().is_empty());
    assert!(io.writes.borrow().is_empty());
}

#[test]
fn changes_only_the_requested_fan_and_preserves_other_pair() {
    let io = Memory::new();
    let before = capture(&io).unwrap();
    let targets = prepare(&before, &BTreeMap::from([("cpu".into(), points())])).unwrap();
    assert_eq!(targets.len(), 1);
    transaction::apply(&io, &before, &targets, 6).unwrap();
    assert_eq!(*io.writes.borrow(), [41]);
    assert_eq!(*io.modes.borrow(), [6]);
    assert_eq!(io.pairs.borrow()[0].tables[1], before.pairs[0].tables[1]);
    assert_eq!(io.pairs.borrow()[1], before.pairs[1]);
    assert_eq!(io.pages.borrow()[4][5], 6);
    assert_eq!(io.pairs.borrow()[0].tables[0][0], 0x82);
}

#[test]
fn silent_rejection_restores_exact_raw_bytes_and_mode() {
    let io = Memory::new();
    let before = capture(&io).unwrap();
    let targets = prepare(&before, &BTreeMap::from([("cpu".into(), points())])).unwrap();
    io.corrupt_once.set(true);
    let error = transaction::apply(&io, &before, &targets, 6)
        .unwrap_err()
        .to_string();
    assert!(error.contains("回读不一致") && error.contains("已恢复"));
    assert_eq!(*io.pairs.borrow(), before.pairs);
    assert_eq!(*io.modes.borrow(), [6, 0]);
    assert_eq!(io.pages.borrow()[4][5], 0);
}

#[test]
fn second_pair_partial_failure_restores_both_pairs() {
    let io = Memory::new();
    let before = capture(&io).unwrap();
    let targets = prepare(
        &before,
        &BTreeMap::from([("cpu".into(), points()), ("sys".into(), points())]),
    )
    .unwrap();
    io.fail_once.set(Some(43));
    assert!(transaction::apply(&io, &before, &targets, 6).is_err());
    assert_eq!(*io.pairs.borrow(), before.pairs);
    assert_eq!(*io.writes.borrow(), [41, 43, 41, 43]);
}

#[test]
fn settings_failure_restores_written_pair_and_reports_recovery_failure() {
    let io = Memory::new();
    let before = capture(&io).unwrap();
    let targets = prepare(&before, &BTreeMap::from([("cpu".into(), points())])).unwrap();
    io.fail_save.set(true);
    assert!(
        transaction::apply(&io, &before, &targets, 6)
            .unwrap_err()
            .to_string()
            .contains("已恢复")
    );
    assert_eq!(*io.pairs.borrow(), before.pairs);
    io.corrupt_once.set(true);
    io.fail_after.set(io.writes.borrow().len() + 1);
    let error = transaction::apply(&io, &before, &targets, 6)
        .unwrap_err()
        .to_string();
    assert!(error.contains("恢复失败") && error.contains("模拟恢复传输失败"));
}

#[test]
fn default_reset_finishes_in_verified_automatic_mode() {
    let io = Memory::new();
    let before = capture(&io).unwrap();
    let mut targets = before.pairs.clone();
    targets[0].tables[0] = rpm::encode_default(&targets[0].tables[0], 0, 2).unwrap();
    transaction::apply(&io, &before, &targets, 0).unwrap();
    assert_eq!(*io.modes.borrow(), [6, 0]);
    assert_eq!(io.pages.borrow()[4][5], 0);
    assert_eq!(&io.pairs.borrow()[0].tables[0][62..64], &[0, 0]);
}
