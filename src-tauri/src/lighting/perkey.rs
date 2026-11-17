//! MCU 逐键协议与原始效果恢复；矩阵从 page2/3 逐页读取，不越过 HAL 页界。
use super::{protocol::Rgb, *};
use sha2::{Digest, Sha256};
#[path = "perkey/controller.rs"]
mod controller;
#[path = "perkey/device.rs"]
mod device;
#[path = "perkey/effects.rs"]
mod effects;
#[path = "perkey/events.rs"]
mod events;
#[path = "perkey/model.rs"]
mod model;
#[path = "perkey/runtime.rs"]
mod runtime;
pub use controller::Controller;
pub use device::DeviceInfo;
pub use model::{Pattern, Profile, Request, Target, Values};
#[path = "perkey/scope.rs"]
mod scope;
#[path = "perkey/session.rs"]
mod session;
pub(crate) use self::scope::{Plan, Scope};
#[cfg(test)]
pub(crate) use runtime::TestRuntime;
#[path = "../hal/perkey_hid.rs"]
pub mod hid;
#[cfg(test)]
#[path = "perkey/tests.rs"]
mod tests;

#[derive(Clone, PartialEq, Eq)]
struct Snapshot {
    settings: Vec<u8>,
    matrix: Vec<u8>,
}
impl Snapshot {
    fn at(&self, offset: usize) -> u8 {
        self.settings[offset - 32]
    }
    fn put(&mut self, offset: usize, value: u8) {
        self.settings[offset - 32] = value;
    }
    fn color(&mut self, offset: usize, color: Rgb) {
        self.settings[offset - 32..offset - 29].copy_from_slice(&[color.r, color.g, color.b]);
    }
    fn rgb(&self, offset: usize) -> Rgb {
        Rgb {
            r: self.at(offset),
            g: self.at(offset + 1),
            b: self.at(offset + 2),
        }
    }
    fn colors(&self) -> Vec<Rgb> {
        (0..120)
            .map(|i| {
                let at = matrix_offset(i);
                Rgb {
                    r: self.matrix[at],
                    g: self.matrix[at + 1],
                    b: self.matrix[at + 2],
                }
            })
            .collect()
    }
    fn revision(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest([self.settings.as_slice(), self.matrix.as_slice()].concat())
        )
    }
    fn validate(&self) -> Result<()> {
        if self.settings.len() != 76 || self.matrix.len() != 384 {
            anyhow::bail!("逐键快照长度错误");
        }
        if self.at(35) > 4 || self.at(34) > 2 || self.at(33) > 1 || self.at(80) > 1 {
            anyhow::bail!("逐键亮度或开关值未知，不能保证恢复");
        }
        if self.at(33) == 1 || self.at(32) == 12 {
            return Ok(());
        }
        effects::dynamic(self)?;
        if (self.at(32) == 5 && self.at(48) > 3)
            || [(1, 52), (2, 56), (8, 69), (9, 76), (13, 41)]
                .iter()
                .any(|(mode, at)| self.at(32) == *mode && self.at(*at) > 1)
        {
            anyhow::bail!("未知逐键动态颜色标志，不能保证恢复");
        }
        Ok(())
    }
}
fn matrix_offset(index: usize) -> usize {
    let (row, col) = (index / 20, index % 20);
    row * 64 + col * 3 + col / 5
}
pub(crate) trait Io {
    fn identity(&self) -> String;
    fn device(&self) -> device::DeviceInfo;
    fn read(&self, page: i32, offset: i32, length: usize) -> Result<Vec<u8>>;
    fn write(&self, page: i32, offset: i32, bytes: &[u8]) -> Result<()>;
    fn send(&self, command: [u8; 5]) -> Result<()>;
    fn settle(&self) {}
}
#[cfg(not(test))]
struct NativeIo<'a> {
    hal: &'a Hal,
    keyboard: hid::Keyboard,
    info: device::DeviceInfo,
}
#[cfg(not(test))]
impl Io for NativeIo<'_> {
    fn identity(&self) -> String {
        self.keyboard.identity()
    }
    fn device(&self) -> device::DeviceInfo {
        self.info.clone()
    }
    fn read(&self, p: i32, o: i32, n: usize) -> Result<Vec<u8>> {
        self.hal.with_dchu(|d| d.read_settings(p, o, n))
    }
    fn write(&self, p: i32, o: i32, b: &[u8]) -> Result<()> {
        self.hal.with_dchu(|d| d.write_settings(p, o, b))
    }
    fn send(&self, command: [u8; 5]) -> Result<()> {
        self.keyboard.send(command)
    }
    fn settle(&self) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
fn read(io: &(impl Io + ?Sized)) -> Result<Snapshot> {
    let snapshot = Snapshot {
        settings: io.read(2, 32, 76)?,
        matrix: [io.read(2, 128, 128)?, io.read(3, 0, 256)?].concat(),
    };
    snapshot.validate()?;
    Ok(snapshot)
}
fn emit(io: &(impl Io + ?Sized), s: &Snapshot) -> Result<()> {
    if s.at(33) == 1 || s.at(32) == 12 {
        io.send([0, 12, 0, 0, 0])?;
        io.settle();
        for (i, c) in s.colors().iter().enumerate() {
            io.send([1, ((i / 20) << 5 | i % 20) as u8, c.r, c.g, c.b])?;
        }
    } else {
        for command in effects::dynamic(s)? {
            io.send(command)?;
        }
    }
    let brightness = if s.at(80) == 0 {
        0
    } else {
        [0, 2, 4, 6, 10][s.at(35) as usize]
    };
    io.send([
        9,
        brightness,
        if s.at(80) == 0 { 0 } else { s.at(34) },
        0,
        0,
    ])
}
fn save(io: &(impl Io + ?Sized), s: &Snapshot, all: bool) -> Result<()> {
    let mut errors = Vec::new();
    let mut regions: Vec<(i32, i32, &[u8])> = [(32usize, 4), (41, 7), (48, 32), (80, 1)]
        .into_iter()
        .map(|(offset, length)| {
            (
                2,
                offset as i32,
                &s.settings[offset - 32..offset - 32 + length],
            )
        })
        .collect();
    regions.extend([(2, 128, &s.matrix[..128]), (3, 0, &s.matrix[128..])]);
    for (page, offset, bytes) in regions {
        let result = io.write(page, offset, bytes).and_then(|_| {
            if io.read(page, offset, bytes.len())? != bytes {
                anyhow::bail!("逐键保存回读不一致 ({page}:{offset})");
            }
            Ok(())
        });
        if let Err(e) = result {
            errors.push(format!("{e:#}"));
            if !all {
                break;
            }
        }
    }
    if !errors.is_empty() {
        anyhow::bail!(errors.join("；"));
    }
    Ok(())
}
fn restore(io: &(impl Io + ?Sized), s: &Snapshot) -> Result<()> {
    let errors = [emit(io, s), save(io, s, true)]
        .into_iter()
        .filter_map(|r| r.err().map(|e| format!("{e:#}")))
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        anyhow::bail!(errors.join("；"));
    }
    Ok(())
}
fn revision(io: &(impl Io + ?Sized), snapshot: &Snapshot) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}:{:?}:{}",
                io.identity(),
                io.device(),
                snapshot.revision()
            )
            .as_bytes()
        )
    )
}
fn binding(io: &(impl Io + ?Sized), raw: u8) -> String {
    format!("{raw}:{}:{:?}", io.identity(), io.device())
}
fn detected(hal: &Hal) -> Result<Option<u8>> {
    hal.with_dchu(|d| {
        let live = d.read_package(13).ok().map(|p| p[15]);
        let cache = d
            .read_settings(2, 4, 1)
            .ok()
            .and_then(|b| b.first().copied());
        let (raw, family, _, _) = super::select_detection(live, cache);
        if family == KeyboardFamily::Unknown {
            anyhow::bail!("键盘类型未知，无法判定逐键能力");
        }
        Ok(if family == KeyboardFamily::Perkey {
            raw
        } else {
            None
        })
    })
}
#[cfg(not(test))]
fn native(hal: &Hal) -> Result<(u8, NativeIo<'_>)> {
    let raw = detected(hal)?.ok_or_else(|| anyhow!("当前设备未确认是逐键键盘"))?;
    let keyboard = hid::Keyboard::open()?;
    let mut info = device::decode(keyboard.query(1)?, keyboard.query(16)?);
    info.compact = hal
        .with_dchu(|d| {
            let mut request = [0; 256];
            request[0] = 12;
            Ok(device::compact(&d.exchange_package(4, &request)?))
        })
        .ok()
        .flatten();
    info.physical_three_column = hal
        .with_dchu(|d| {
            Ok(match d.read_package(17)?[14] {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            })
        })
        .ok()
        .flatten();
    Ok((
        raw,
        NativeIo {
            hal,
            keyboard,
            info,
        },
    ))
}
#[cfg(test)]
fn apply(
    io: &impl Io,
    baseline: &mut Option<(String, Snapshot)>,
    raw: u8,
    request: &Request,
) -> Result<()> {
    let before = read(io)?;
    let target = model::target(&before, &request.values, &io.device())?;
    if revision(io, &before) != request.expected_revision {
        anyhow::bail!("逐键保存值或设备已变化，请重新读取并确认");
    }
    write_target(
        io,
        baseline,
        raw,
        &before,
        &target,
        request.values.enabled && request.values.pattern.reactive(),
        &mut false,
    )
}
fn write_target(
    io: &(impl Io + ?Sized),
    baseline: &mut Option<(String, Snapshot)>,
    raw: u8,
    before: &Snapshot,
    target: &Snapshot,
    replay: bool,
    needs_rollback: &mut bool,
) -> Result<()> {
    let identity = binding(io, raw);
    if baseline.as_ref().is_some_and(|(kind, _)| *kind != identity) {
        anyhow::bail!("键盘身份与初始快照不一致");
    }
    if before == target && !replay {
        return Ok(());
    }
    baseline.get_or_insert_with(|| (identity, before.clone()));
    *needs_rollback = true;
    if let Err(error) = emit(io, target).and_then(|_| save(io, target, false)) {
        let recovery = restore(io, before).map_err(|e| format!("{e:#}"));
        if recovery.is_ok() {
            *needs_rollback = false;
        }
        anyhow::bail!(ccc_core::msr::with_recovery(
            format!("逐键写入失败：{error:#}"),
            recovery
        ));
    }
    Ok(())
}
