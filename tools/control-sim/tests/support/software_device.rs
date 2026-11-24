use super::*;
use hal::{firmware::DirectControl, oem_dchu::Dchu};
use std::sync::mpsc::{Receiver, SyncSender};

const CURVE: [u8; 14] = [40, 60, 80, 95, 51, 102, 204, 255, 0, 0, 0, 0, 0, 0];

pub(super) struct Device {
    pub pages: [[u8; 256]; 16],
    pub cpu: u8,
    pub writes: Vec<(i32, Vec<u8>)>,
    pub fail_duty: bool,
    pub fail_release: bool,
    pub ignore_duty: bool,
    pub ignore_release: bool,
    pub release_hold: Option<(SyncSender<()>, Receiver<()>)>,
    pub journal_path: Option<PathBuf>,
    pub unjournaled: usize,
    pub hold_on_duty: bool,
    pub journal_hold: Option<fs::File>,
    pub control: [DirectControl; 2],
    pub mode: u8,
    offset: u8,
    curves: [[u8; 14]; 3],
}
impl Device {
    pub(super) const fn new() -> Self {
        let mut pages = [[0; 256]; 16];
        pages[1][1] = 3;
        pages[7][0] = 1;
        pages[7][2] = 1;
        pages[7][17] = 15;
        Self {
            pages,
            cpu: 50,
            writes: Vec::new(),
            fail_duty: false,
            fail_release: false,
            ignore_duty: false,
            ignore_release: false,
            release_hold: None,
            journal_path: None,
            unjournaled: 0,
            hold_on_duty: false,
            journal_hold: None,
            control: [DirectControl::Firmware; 2],
            mode: 0,
            offset: 0,
            curves: [CURVE, CURVE, [0; 14]],
        }
    }

    fn write_duty(&mut self, bytes: &[u8]) -> bool {
        let active = self
            .journal_path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|record| record["active"] == true);
        if !active {
            self.unjournaled += 1;
        }
        if self.hold_on_duty {
            use std::os::windows::fs::OpenOptionsExt;
            self.hold_on_duty = false;
            self.journal_hold = self.journal_path.as_ref().and_then(|path| {
                fs::OpenOptions::new()
                    .read(true)
                    .share_mode(1)
                    .open(path)
                    .ok()
            });
        }
        if std::mem::take(&mut self.fail_duty) {
            return false;
        }
        if !self.ignore_duty {
            self.control[usize::from(bytes[1] - 1)] = DirectControl::Manual(bytes[2]);
        }
        true
    }

    fn write_curve(&mut self, bytes: &[u8]) {
        for (index, curve) in self.curves.iter_mut().enumerate() {
            let points = 2 + index * 4;
            let slopes = 14 + index * 6;
            [curve[1], curve[5], curve[2], curve[6]] =
                bytes[points..points + 4].try_into().unwrap();
            curve[8..14].copy_from_slice(&bytes[slopes..slopes + 6]);
        }
    }
}

pub(super) static DEVICE: Mutex<Device> = Mutex::new(Device::new());

pub(super) fn direct_control(_: &Dchu) -> anyhow::Result<[DirectControl; 2]> {
    Ok(DEVICE.lock().unwrap().control)
}

pub(super) fn io() -> Dchu {
    Dchu::test_io(buffer, read, set, write).with_test_exchange(exchange)
}

unsafe extern "system" fn buffer(command: i32, out: *mut u8) -> i32 {
    let device = DEVICE.lock().unwrap();
    let mut p = [0; 257];
    match command {
        12 => {
            p[18] = device.cpu;
            p[16] = 76;
            p[19] = 76;
            p[2..4].copy_from_slice(&474u16.to_be_bytes());
        }
        13 => {
            p[12] = 2;
            p[14] = match device.mode {
                0 => 0x02,
                1 => 0x10,
                2 => 0x08,
                5 => 0x01,
                6 => 0x04,
                7 => 0x20,
                8 => 0x40,
                9 => 0x80,
                _ => 0,
            };
            p[50] = 3;
            for (i, curve) in device.curves.iter().enumerate() {
                let at = 16 + i * 8;
                for point in 0..4 {
                    p[at + point * 2] = curve[point];
                    p[at + point * 2 + 1] = curve[4 + point];
                }
            }
        }
        _ => return 0,
    }
    p[256] = p[0];
    unsafe {
        std::ptr::copy_nonoverlapping(p.as_ptr(), out, p.len());
    }
    command
}
unsafe extern "system" fn exchange(
    command: i32,
    input: *const u8,
    length: i32,
    out: *mut u8,
) -> i32 {
    assert_eq!((command, length), (4, 256));
    let device = DEVICE.lock().unwrap();
    let mut p = [0; 257];
    match unsafe { *input } {
        3 => {
            p[0x07] = device.cpu;
            p[0xe7] = device.offset;
        }
        5 => {
            p[0x0a] = 2;
            for (index, curve) in device.curves.iter().enumerate() {
                let start = 0x0c + index * 14;
                p[start..start + 14].copy_from_slice(curve);
            }
        }
        8 => p[..256].copy_from_slice(&device.pages[7]),
        _ => return 0,
    }
    p[256] = p[0];
    unsafe {
        std::ptr::copy_nonoverlapping(p.as_ptr(), out, p.len());
    }
    command
}
unsafe extern "system" fn read(page: i32, offset: i32, length: i32, out: *mut u8) -> i32 {
    let device = DEVICE.lock().unwrap();
    unsafe {
        std::ptr::copy_nonoverlapping(
            device.pages[page as usize][offset as usize..].as_ptr(),
            out,
            length as usize,
        );
    }
    4096
}
unsafe extern "system" fn write(page: i32, offset: i32, length: i32, data: *const u8) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts(data, length as usize) };
    let mut device = DEVICE.lock().unwrap();
    device.pages[page as usize][offset as usize..offset as usize + bytes.len()]
        .copy_from_slice(bytes);
    1
}
unsafe extern "system" fn set(command: i32, data: *const u8, length: i32) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts(data, length as usize) };
    let mut device = DEVICE.lock().unwrap();
    device.writes.push((command, bytes.to_vec()));
    match command {
        4 if bytes.get(6) == Some(&0xc1) => {
            if !device.write_duty(bytes) {
                return 0;
            }
        }
        0x69 => {
            if let Some((entered, resume)) = device.release_hold.take() {
                let _ = entered.send(());
                let _ = resume.recv_timeout(Duration::from_secs(7));
            }
            if std::mem::take(&mut device.fail_release) {
                return 0;
            }
            if !device.ignore_release {
                for (index, control) in device.control.iter_mut().enumerate() {
                    if bytes[0] & (1 << index) != 0 {
                        *control = DirectControl::Firmware;
                    }
                }
            }
        }
        14 => {
            device.write_curve(bytes);
            return 0x14;
        }
        121 if bytes[3] == 1 => device.mode = bytes[0],
        121 if bytes[3] == 14 => device.offset = bytes[0],
        121 if bytes[3] == 34 => device.curves = [CURVE, CURVE, [0; 14]],
        _ => {}
    }
    command
}
