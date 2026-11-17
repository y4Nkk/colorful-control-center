//! Intel RAPL / OC Mailbox 协议的唯一所有者；调用方提供传输，测试不访问硬件。

pub const PAWNIO_DEVICE_PATH: [u16; 11] = [92, 92, 46, 92, 80, 97, 119, 110, 73, 79, 0];
pub const MSR_RAPL_POWER_UNIT: u32 = 0x606;
pub const MSR_PKG_POWER_LIMIT: u32 = 0x610;
pub const OC_MAILBOX: u32 = 0x150;
const BUSY: u64 = 1 << 63;
const POWER_FIELDS: u64 = 0xffff | (0xffff << 32);

pub trait MsrAccess {
    fn read(&self, address: u32) -> Result<u64, String>;
    fn write(&self, address: u32, value: u64) -> Result<(), String>;
    fn pause(&self) {}
}

pub fn power_unit(rapl: u64) -> f64 {
    2f64.powi(-((rapl & 15) as i32))
}

pub fn decode_power_limits(value: u64, unit: f64) -> (f64, f64) {
    (
        (value & 0x7fff) as f64 * unit,
        ((value >> 32) & 0x7fff) as f64 * unit,
    )
}

pub fn encode_power_limit(
    current: u64,
    pl1: Option<f64>,
    pl2: Option<f64>,
    unit: f64,
) -> Result<u64, String> {
    if !unit.is_finite() || unit <= 0.0 {
        return Err("RAPL 功率单位无效".into());
    }
    let mut value = current;
    for (wanted, shift) in [(pl1, 0), (pl2, 32)] {
        if let Some(watts) = wanted {
            let units = (watts / unit).round();
            if !watts.is_finite() || !(1.0..=32767.0).contains(&units) {
                return Err("功耗上限不能表示为有效 RAPL 数值".into());
            }
            value = (value & !(0x7fff << shift)) | ((units as u64 | 0x8000) << shift);
        }
    }
    Ok(value)
}

pub fn power_field_mask(pl1: bool, pl2: bool) -> u64 {
    (if pl1 { 0xffff } else { 0 }) | (if pl2 { 0xffff << 32 } else { 0 })
}

pub fn apply_power(io: &impl MsrAccess, pl1: Option<f64>, pl2: Option<f64>) -> Result<u64, String> {
    if pl1.is_none() && pl2.is_none() {
        return Err("没有提供功耗上限变更".into());
    }
    let before = io.read(MSR_PKG_POWER_LIMIT)?;
    if before & BUSY != 0 {
        return Err("MSR 0x610 bit63 已锁定，未执行写入".into());
    }
    let unit = power_unit(io.read(MSR_RAPL_POWER_UNIT)?);
    let value = encode_power_limit(before, pl1, pl2, unit)?;
    let mask = power_field_mask(pl1.is_some(), pl2.is_some());
    let result = write_power_verified(io, value, mask);
    if let Err(error) = result {
        return Err(with_recovery(
            error,
            write_power_verified(io, before, POWER_FIELDS),
        ));
    }
    Ok(before)
}

pub fn write_power_verified(io: &impl MsrAccess, value: u64, mask: u64) -> Result<(), String> {
    io.write(MSR_PKG_POWER_LIMIT, value)?;
    let actual = io.read(MSR_PKG_POWER_LIMIT)?;
    if actual & mask != value & mask {
        return Err(format!(
            "功耗上限回读不一致：期望 {value:#x}，实际 {actual:#x}"
        ));
    }
    Ok(())
}

pub fn restore_power(io: &impl MsrAccess, before: u64) -> Result<(), String> {
    let current = io.read(MSR_PKG_POWER_LIMIT)?;
    if current & POWER_FIELDS == before & POWER_FIELDS {
        return Ok(());
    }
    if current & BUSY != 0 {
        return Err("功耗上限现已锁定，无法恢复".into());
    }
    write_power_verified(
        io,
        (current & !POWER_FIELDS) | (before & POWER_FIELDS),
        POWER_FIELDS,
    )
}

pub fn offset_units(mv: f64) -> Result<i16, String> {
    let units = (mv * 1.024).round();
    if !mv.is_finite() || !(-1024.0..=1023.0).contains(&units) {
        return Err("电压偏移超出 OC Mailbox 11 位有符号范围".into());
    }
    Ok(units as i16)
}

pub fn decode_offset(response: u64) -> i16 {
    let raw = ((response >> 21) & 0x7ff) as i16;
    (raw << 5) >> 5
}

pub fn mailbox_command(plane: u64, units: Option<i16>) -> Result<u64, String> {
    if plane > 4 {
        return Err("未知电压域".into());
    }
    let command = BUSY | (plane << 40) | (0x10 << 32);
    Ok(match units {
        None => command,
        Some(value) => command | (1 << 32) | (((value as u16 & 0x7ff) as u64) << 21),
    })
}

fn wait_mailbox(io: &impl MsrAccess) -> Result<u64, String> {
    for _ in 0..100 {
        let response = io.read(OC_MAILBOX)?;
        if response & BUSY == 0 {
            return Ok(response);
        }
        io.pause();
    }
    Err("OC Mailbox busy 超时".into())
}

fn send_mailbox(io: &impl MsrAccess, command: u64) -> Result<u64, String> {
    wait_mailbox(io)?;
    io.write(OC_MAILBOX, command)?;
    let response = wait_mailbox(io)?;
    let status = (response >> 32) & 0xff;
    if status != 0 {
        return Err(format!("OC Mailbox 返回状态 {status:#x}"));
    }
    Ok(response)
}

pub fn read_voltage_units(io: &impl MsrAccess, plane: u64) -> Result<i16, String> {
    Ok(decode_offset(send_mailbox(
        io,
        mailbox_command(plane, None)?,
    )?))
}

pub fn write_voltage_units(io: &impl MsrAccess, plane: u64, units: i16) -> Result<(), String> {
    send_mailbox(io, mailbox_command(plane, Some(units))?)?;
    let actual = read_voltage_units(io, plane)?;
    if actual != units {
        return Err(format!(
            "电压偏移回读不一致：期望 {units} 单位，实际 {actual} 单位"
        ));
    }
    Ok(())
}

pub fn apply_voltage(io: &impl MsrAccess, plane: u64, mv: f64) -> Result<i16, String> {
    let target = offset_units(mv)?;
    let before = read_voltage_units(io, plane)?;
    if let Err(error) = write_voltage_units(io, plane, target) {
        return Err(with_recovery(error, write_voltage_units(io, plane, before)));
    }
    Ok(before)
}

#[derive(Debug, PartialEq)]
pub struct VoltageProbe {
    pub before_units: i16,
    pub target_units: i16,
}

impl VoltageProbe {
    pub fn negative_offset_verified(&self) -> bool {
        self.target_units < 0
    }
}

/// 只降低一编码单位；无论探测结果如何都尝试恢复，成功也必须恢复回读一致。
pub fn probe_voltage_write(io: &impl MsrAccess, plane: u64) -> Result<VoltageProbe, String> {
    let before = read_voltage_units(io, plane)?;
    if before == -1024 {
        return Err("当前已在负偏移编码下界，未执行探测写入".into());
    }
    let target = before - 1;
    let tested = write_voltage_units(io, plane, target);
    let restored = write_voltage_units(io, plane, before);
    match (tested, restored) {
        (Ok(()), Ok(())) => Ok(VoltageProbe {
            before_units: before,
            target_units: target,
        }),
        (Err(error), recovery) => Err(with_recovery(error, recovery)),
        (Ok(()), Err(error)) => Err(format!("探测写入完成，但恢复失败：{error}")),
    }
}

pub fn with_recovery(error: String, recovery: Result<(), String>) -> String {
    match recovery {
        Ok(()) => format!("{error}；已恢复写入前状态"),
        Err(restore) => format!("{error}；恢复失败：{restore}"),
    }
}

#[cfg(test)]
#[path = "msr/probe_tests.rs"]
mod probe_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Mock {
        power: Cell<u64>,
        voltage: Cell<i16>,
        response: Cell<u64>,
        ignore: Cell<bool>,
        writes: RefCell<Vec<(u32, u64)>>,
    }

    impl Default for Mock {
        fn default() -> Self {
            Self {
                power: Cell::new(0),
                voltage: Cell::new(0),
                response: Cell::new(0),
                ignore: Cell::new(false),
                writes: RefCell::new(Vec::new()),
            }
        }
    }

    impl MsrAccess for Mock {
        fn read(&self, address: u32) -> Result<u64, String> {
            Ok(match address {
                MSR_RAPL_POWER_UNIT => 3,
                MSR_PKG_POWER_LIMIT => self.power.get(),
                OC_MAILBOX => self.response.get(),
                _ => 0,
            })
        }
        fn write(&self, address: u32, value: u64) -> Result<(), String> {
            self.writes.borrow_mut().push((address, value));
            if address == MSR_PKG_POWER_LIMIT && !self.ignore.get() {
                self.power.set(value);
            }
            if address == OC_MAILBOX {
                if (value >> 32) & 0xff == 0x11 && !self.ignore.get() {
                    self.voltage.set(decode_offset(value));
                }
                self.response
                    .set(((self.voltage.get() as u16 & 0x7ff) as u64) << 21);
            }
            Ok(())
        }
    }

    #[test]
    fn device_path_has_only_terminal_nul() {
        assert!(PAWNIO_DEVICE_PATH[..10].iter().all(|v| *v != 0));
        assert_eq!(
            String::from_utf16(&PAWNIO_DEVICE_PATH[..10]).unwrap(),
            r"\\.\PawnIO"
        );
    }

    #[test]
    fn power_uses_rapl_units_and_preserves_unrelated_bits() {
        let before = 0x0042_0000_0064_0000;
        let after = encode_power_limit(before, Some(60.0), Some(90.0), power_unit(4)).unwrap();
        assert_eq!(decode_power_limits(after, 0.0625), (60.0, 90.0));
        assert_eq!(after & !POWER_FIELDS, before & !POWER_FIELDS);
        assert_eq!(after & (0x8000 | (0x8000 << 32)), 0x8000 | (0x8000 << 32));
    }

    #[test]
    fn locked_power_does_not_write() {
        let io = Mock::default();
        io.power.set(BUSY);
        assert!(apply_power(&io, Some(60.0), None).is_err());
        assert!(io.writes.borrow().is_empty());
    }

    #[test]
    fn already_restored_locked_power_confirms_fields_without_writing() {
        let io = Mock::default();
        let before = 0x8321_0000_8211;
        io.power.set(before | BUSY);
        restore_power(&io, before).unwrap();
        assert!(io.writes.borrow().is_empty());
        assert!(restore_power(&io, before + 1).unwrap_err().contains("锁定"));
        assert!(io.writes.borrow().is_empty());
    }

    #[test]
    fn power_verifies_both_fields_and_rolls_back_on_ignored_write() {
        let io = Mock::default();
        io.ignore.set(true);
        let error = apply_power(&io, Some(60.0), Some(90.0)).unwrap_err();
        assert!(error.contains("回读不一致") && error.contains("已恢复"));
        assert_eq!(io.writes.borrow().len(), 2);
    }

    #[test]
    fn voltage_sign_and_raw_restore_are_exact() {
        for units in [-1024, -154, -1, 0, 1, 1023] {
            assert_eq!(
                decode_offset(mailbox_command(0, Some(units)).unwrap()),
                units
            );
        }
        let io = Mock::default();
        io.voltage.set(-51);
        let before = apply_voltage(&io, 0, -80.0).unwrap();
        assert_eq!(before, -51);
        write_voltage_units(&io, 0, before).unwrap();
        assert_eq!(io.voltage.get(), -51);
    }

    #[test]
    fn ignored_one_unit_probe_is_detected() {
        let io = Mock::default();
        io.ignore.set(true);
        assert!(
            apply_voltage(&io, 0, 1.0)
                .unwrap_err()
                .contains("回读不一致")
        );
    }

    #[test]
    fn mailbox_busy_timeout_prevents_write() {
        let io = Mock::default();
        io.response.set(BUSY);
        assert!(read_voltage_units(&io, 0).unwrap_err().contains("超时"));
        assert!(io.writes.borrow().is_empty());
    }

    #[test]
    fn invalid_numbers_never_encode() {
        assert!(offset_units(f64::NAN).is_err());
        assert!(encode_power_limit(0, Some(f64::INFINITY), None, 0.125).is_err());
        assert!(mailbox_command(5, None).is_err());
    }
}
