use super::*;
use std::cell::{Cell, RefCell};

struct Io {
    voltage: Cell<i16>,
    response: Cell<u64>,
    attempted: RefCell<Vec<i16>>,
    fault: u8,
}

impl Io {
    fn new(before: i16, fault: u8) -> Self {
        Self {
            voltage: Cell::new(before),
            response: Cell::new(0),
            attempted: RefCell::new(vec![]),
            fault,
        }
    }
}

impl MsrAccess for Io {
    fn read(&self, _: u32) -> Result<u64, String> {
        Ok(self.response.get())
    }
    fn write(&self, _: u32, value: u64) -> Result<(), String> {
        if (value >> 32) & 0xff == 0x11 {
            let target = decode_offset(value);
            self.attempted.borrow_mut().push(target);
            let attempt = self.attempted.borrow().len();
            match (self.fault, attempt) {
                (1, 1) => {
                    // 写可能生效后报错，也必须恢复。
                    self.voltage.set(target);
                    return Err("injected write failure".into());
                }
                (2, 1) | (3, 2) => (), // 静默忽略探测或恢复。
                (4, _) => return Err("injected write and restore failure".into()),
                _ => self.voltage.set(target),
            }
        }
        self.response
            .set(((self.voltage.get() as u16 & 0x7ff) as u64) << 21);
        Ok(())
    }
}

#[test]
fn probe_uses_one_negative_step_and_restores_exact_encoding() {
    for before in [0, -51, -1023, 1, 1023] {
        let io = Io::new(before, 0);
        let result = probe_voltage_write(&io, 0).unwrap();
        assert_eq!(result.before_units, before);
        assert_eq!(result.target_units, before - 1);
        assert_eq!(result.negative_offset_verified(), before <= 0);
        assert_eq!(io.voltage.get(), before);
        assert_eq!(*io.attempted.borrow(), [before - 1, before]);
    }
}

#[test]
fn lower_boundary_does_not_write_an_offset() {
    let io = Io::new(-1024, 0);
    assert!(probe_voltage_write(&io, 0).unwrap_err().contains("下界"));
    assert!(io.attempted.borrow().is_empty());
}

#[test]
fn probe_write_error_or_ignored_write_always_restores() {
    for fault in [1, 2] {
        let io = Io::new(-51, fault);
        let error = probe_voltage_write(&io, 0).unwrap_err();
        assert!(error.contains("已恢复"), "{error}");
        assert_eq!(*io.attempted.borrow(), [-52, -51]);
        assert_eq!(io.voltage.get(), -51);
    }
}

#[test]
fn failed_restore_is_never_reported_as_probe_success() {
    for fault in [3, 4] {
        let io = Io::new(-51, fault);
        let error = probe_voltage_write(&io, 0).unwrap_err();
        assert!(error.contains("恢复失败"), "{error}");
        assert_eq!(*io.attempted.borrow(), [-52, -51]);
    }
}

#[test]
fn mailbox_read_failure_during_probe_still_attempts_restore() {
    struct ReadFault {
        inner: Io,
        failed: Cell<bool>,
    }
    impl MsrAccess for ReadFault {
        fn read(&self, address: u32) -> Result<u64, String> {
            if self.inner.attempted.borrow().len() == 1 && !self.failed.replace(true) {
                return Err("injected read failure".into());
            }
            self.inner.read(address)
        }
        fn write(&self, address: u32, value: u64) -> Result<(), String> {
            self.inner.write(address, value)
        }
    }
    let io = ReadFault {
        inner: Io::new(-51, 0),
        failed: Cell::new(false),
    };
    let error = probe_voltage_write(&io, 0).unwrap_err();
    assert!(error.contains("injected read failure") && error.contains("已恢复"));
    assert_eq!(*io.inner.attempted.borrow(), [-52, -51]);
    assert_eq!(io.inner.voltage.get(), -51);
}
