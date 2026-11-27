use super::*;
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy)]
enum Fault {
    Transport(usize),
    Temperature(usize),
    Dust,
}

struct Fixture {
    mode: Cell<u8>,
    offset: Cell<u8>,
    channels: u8,
    writes: RefCell<Vec<(u8, u8, bool)>>,
    loaded: Cell<bool>,
    reads: Cell<usize>,
    fault: Fault,
    restore_offset_fails: bool,
}

const BEFORE: Baseline = Baseline {
    mode: 1,
    offset: 0x4d,
    channels: 0b11,
};

impl Fixture {
    fn new(fault: Fault) -> Self {
        Self {
            mode: Cell::new(BEFORE.mode),
            offset: Cell::new(BEFORE.offset),
            channels: BEFORE.channels,
            writes: RefCell::default(),
            loaded: Cell::default(),
            reads: Cell::default(),
            fault,
            restore_offset_fails: false,
        }
    }

    fn execute(&self, samples: &Cell<usize>) -> Result<Option<Vec<Row>>> {
        let args = FanOffsetArgs {
            execute: true,
            offset: 30,
            settle: 0,
            window: 3,
            threads: None,
            load: 30,
            out: String::new(),
        };
        run(
            self,
            BEFORE,
            &super::super::plan(30),
            &args,
            |row| {
                assert_eq!(row.phase, "auto-0");
                samples.set(samples.get() + 1);
                Ok(())
            },
            |_| {
                self.loaded.set(true);
                Ok(FakeLoad(&self.loaded))
            },
        )
    }
}

struct FakeLoad<'a>(&'a Cell<bool>);

impl ActiveLoad for FakeLoad<'_> {
    fn refresh(&self, _: Instant) -> Result<()> {
        assert!(self.0.get());
        Ok(())
    }
}

impl Drop for FakeLoad<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl FanIo for Fixture {
    fn read_package(&self, command: i32) -> Result<[u8; 256]> {
        let mut packet = [0; 256];
        if command == 13 {
            packet[12] = 2;
            packet[50] = self.channels;
            packet[14] = match self.mode.get() {
                1 => 0x10,
                0 => 0x02,
                8 => 0x40,
                mode => panic!("unexpected mode {mode}"),
            };
        } else {
            assert_eq!(command, 12);
            let count = self.reads.get() + 1;
            self.reads.set(count);
            if matches!(self.fault, Fault::Transport(at) if at == count) {
                bail!("injected cmd12 transport loss");
            }
            packet[18] = 55;
            packet[21] = if matches!(self.fault, Fault::Temperature(at) if at == count) {
                0
            } else {
                47
            };
        }
        Ok(packet)
    }

    fn dust_running(&self) -> Result<Option<bool>> {
        if matches!(self.fault, Fault::Dust) {
            bail!("injected dust unknown");
        }
        Ok(None)
    }

    fn read_offset(&self) -> Result<u8> {
        Ok(self.offset.get())
    }

    fn set_mode(&self, mode: u8) -> Result<()> {
        self.writes.borrow_mut().push((1, mode, self.loaded.get()));
        self.mode.set(mode);
        Ok(())
    }

    fn restore_mode(&self, mode: u8) -> Result<()> {
        self.set_mode(mode)
    }

    fn set_offset(&self, raw: u8) -> Result<()> {
        self.writes.borrow_mut().push((14, raw, self.loaded.get()));
        if raw == BEFORE.offset && self.restore_offset_fails {
            bail!("injected recovery failure");
        }
        self.offset.set(raw);
        Ok(())
    }
}

#[test]
fn lost_transport_unloads_before_raw_restore_and_skips_later_phases() {
    let io = Fixture::new(Fault::Transport(4));
    let samples = Cell::new(0);
    let result = io.execute(&samples);
    assert!(format!("{:#}", result.err().unwrap()).contains("cmd12 transport loss"));
    assert_eq!(samples.get(), 1);
    assert_eq!(
        &*io.writes.borrow(),
        &[
            (1, 0, true),
            (14, 0, true),
            (14, BEFORE.offset, false),
            (1, BEFORE.mode, false),
        ]
    );
    assert_eq!(io.offset.get(), BEFORE.offset);
    assert_eq!(io.mode.get(), BEFORE.mode);
}

#[test]
fn invalid_temperature_aborts_even_when_the_packet_is_not_empty() {
    let io = Fixture::new(Fault::Temperature(3));
    let samples = Cell::new(0);
    let result = io.execute(&samples);
    assert!(format!("{:#}", result.err().unwrap()).contains("GPU 1 温度失效"));
    assert_eq!(samples.get(), 0);
    assert_eq!(io.writes.borrow().len(), 4);
    assert!(!io.loaded.get());
}

#[test]
fn initial_missing_temperature_and_unknown_dust_never_write_or_start_load() {
    for issue in [Fault::Temperature(1), Fault::Dust] {
        let io = Fixture::new(issue);
        let result = io.execute(&Cell::new(0));
        assert!(format!("{:#}", result.err().unwrap()).contains("启动前复核失败"));
        assert!(io.writes.borrow().is_empty());
        assert!(!io.loaded.get());
    }
}

#[test]
fn recovery_attempts_original_mode_and_preserves_both_errors_when_offset_fails() {
    let mut io = Fixture::new(Fault::Temperature(3));
    io.restore_offset_fails = true;
    let error = io.execute(&Cell::new(0)).err().unwrap();
    let message = format!("{error:#}");
    assert!(message.contains("GPU 1 温度失效"));
    assert!(message.contains("injected recovery failure"));
    assert_eq!(io.mode.get(), BEFORE.mode);
    assert!(!io.loaded.get());
}

#[test]
fn all_present_channels_are_checked_for_missing_temperature_and_heat() {
    let mut packet = [0; 256];
    for (at, temp) in [(18, 60), (21, 55), (24, 50), (40, 45)] {
        packet[at] = temp;
    }
    assert!(decode_sample(&packet, 0b1111).is_ok());
    packet[40] = 95;
    let diagnostic = decode_sample(&packet, 0b1111).err().unwrap().to_string();
    assert!(diagnostic.contains("系统风扇 温度 95°C"));
    packet[24] = 0xff;
    let diagnostic = decode_sample(&packet, 0b1111).err().unwrap().to_string();
    assert!(diagnostic.contains("GPU 2 温度失效"));
}
