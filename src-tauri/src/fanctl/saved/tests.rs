use super::*;
use std::cell::RefCell;
struct Memory(RefCell<[[u8; 256]; 8]>);
impl SettingsAccess for Memory {
    fn read(&self, page: u8, offset: usize, length: usize) -> Result<Vec<u8>, String> {
        Ok(self.0.borrow()[page as usize][offset..offset + length].to_vec())
    }
    fn write(&self, page: u8, offset: usize, bytes: &[u8]) -> Result<(), String> {
        self.0.borrow_mut()[page as usize][offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}
#[test]
fn mode_and_offset_share_oem_slots_and_full_speed_is_not_remembered() {
    let mut pages = [[0; 256]; 8];
    pages[7][0] = 1;
    pages[7][20] = 0x40;
    let io = Memory(RefCell::new(pages));
    for (power_mode, mode_at, offset_at) in [
        (0, 70, 74),
        (1, 71, 75),
        (2, 72, 76),
        (3, 73, 77),
        (4, 70, 74),
    ] {
        io.0.borrow_mut()[1][1] = power_mode;
        let fields = mode(&io, 1).unwrap();
        assert_eq!(fields, [Patch::new(5, [1]), Patch::new(mode_at, [0])]);
        assert_eq!(
            mode(&io, 6).unwrap(),
            [Patch::new(5, [6]), Patch::new(mode_at, [6])]
        );
        assert_eq!(
            offset(slot(&io).unwrap(), 35),
            [Patch::new(7, [35]), Patch::new(offset_at, [35])]
        );
    }
}
#[test]
fn no_capability_changes_no_slots_and_unknown_mode_never_selects_a_slot() {
    let io = Memory(RefCell::new([[0; 256]; 8]));
    assert_eq!(mode(&io, 0).unwrap(), [Patch::new(5, [0])]);
    assert!(capture_slots(&io).unwrap().is_empty());
    io.0.borrow_mut()[7][0] = 1;
    io.0.borrow_mut()[7][20] = 0x10;
    io.0.borrow_mut()[1][1] = 255;
    assert!(slot(&io).is_err());
}
#[test]
fn fields_and_memory_slot_roll_back_together_on_partial_write_failure() {
    struct FailOnce {
        inner: Memory,
        failed: std::cell::Cell<bool>,
    }
    impl SettingsAccess for FailOnce {
        fn read(&self, p: u8, o: usize, n: usize) -> Result<Vec<u8>, String> {
            self.inner.read(p, o, n)
        }
        fn write(&self, p: u8, o: usize, b: &[u8]) -> Result<(), String> {
            self.inner.write(p, o, b)?;
            if o == 70 && !self.failed.replace(true) {
                return Err("保存槽部分写入后失败".into());
            }
            Ok(())
        }
    }
    let mut pages = [[0; 256]; 8];
    pages[7][0] = 1;
    pages[7][20] = 0x40;
    pages[4][5] = 8;
    pages[4][70] = 8;
    let io = FailOnce {
        inner: Memory(RefCell::new(pages)),
        failed: std::cell::Cell::new(false),
    };
    assert!(ccc_core::settings::apply(&io, 4, &mode(&io, 6).unwrap(), None).is_err());
    assert_eq!(*io.inner.0.borrow(), pages);
}
