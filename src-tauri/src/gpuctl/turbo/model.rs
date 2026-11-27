use super::source::Source;
use crate::hal::nvapi::{
    clock::{LockEntry, LockTable},
    pstates::ParsedPstates,
    transaction::RawOffsets,
    turbo::{DeviceId, TurboIo},
};
use anyhow::{Context, Result, bail};
use ccc_core::gpu_turbo::{self, Target};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct Snapshot {
    pub source: Source,
    pub devices: Vec<DeviceId>,
    pub locks: LockTable,
    pub offsets: Vec<ParsedPstates>,
    pub base_mhz: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceReading {
    pub id: DeviceId,
    pub offsets: RawOffsets,
    pub core_editable: bool,
    pub memory_editable: bool,
    pub core_range_khz: [i32; 2],
    pub memory_range_khz: [i32; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeTarget {
    pub mode: u8,
    pub on_ac: bool,
    pub target: Target,
    pub unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Current {
    pub source: Source,
    pub devices: Vec<DeviceReading>,
    pub other_devices: Vec<DeviceId>,
    pub locks: [LockEntry; 2],
    pub base_mhz: Option<u32>,
    pub off: Target,
    pub off_error: Option<String>,
    pub current_on: Option<Target>,
    pub on_targets: Vec<ModeTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Initial {
    pub devices: Vec<DeviceId>,
    pub locks: Option<[LockEntry; 2]>,
    pub offsets: Vec<RawOffsets>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReading {
    pub locks: Option<[LockEntry; 2]>,
    pub offsets: Vec<RawOffsets>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub identity: Option<String>,
    pub current: Option<Current>,
    pub error: Option<String>,
    pub initial: Option<Initial>,
    pub restore_current: Option<RestoreReading>,
    pub restore_revision: Option<String>,
    pub restore_error: Option<String>,
}

pub fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

pub fn raw(parsed: &ParsedPstates) -> RawOffsets {
    RawOffsets {
        core_khz: parsed.core.present.then_some(parsed.core.delta_khz),
        mem_khz: parsed.memory.present.then_some(parsed.memory.delta_khz),
    }
}

pub(super) fn capabilities(value: &ParsedPstates) -> impl Serialize {
    (
        value.editable,
        value.p0_present,
        value.count_clamped,
        (
            value.core.present,
            value.core.editable,
            value.core.min_khz,
            value.core.max_khz,
        ),
        (
            value.memory.present,
            value.memory.editable,
            value.memory.min_khz,
            value.memory.max_khz,
        ),
    )
}
pub fn capture(io: &dyn TurboIo, source: Source) -> Result<Snapshot> {
    capture_with_base(io, source, true)
}

pub(super) fn capture_with_base(
    io: &dyn TurboIo,
    source: Source,
    read_base: bool,
) -> Result<Snapshot> {
    let devices = io.devices().context("读取 GPU 设备集合")?;
    if devices.is_empty() {
        bail!("未枚举到 NVIDIA 物理 GPU");
    }
    if io.primary()? != devices[0] {
        bail!("GPU 控制会话与当前第一张显卡不一致");
    }
    for (index, id) in devices.iter().enumerate() {
        if devices[..index].contains(id) {
            bail!("GPU 设备身份重复");
        }
    }
    let locks = io.locks(&devices[0]).context("读取 GPU 原厂锁频")?;
    locks.owned()?;
    let offsets = devices
        .iter()
        .take(2)
        .map(|id| io.offsets(id))
        .collect::<Result<Vec<_>>>()?;
    let base_mhz = if read_base && source.limits.thermal_mhz == 0 {
        Some(
            io.base_clock(&devices[0])
                .context("读取 GPU 原厂基础频率")?,
        )
    } else {
        None
    };
    Ok(Snapshot {
        source,
        devices,
        locks,
        offsets,
        base_mhz,
    })
}

impl Snapshot {
    pub fn identity(&self) -> Result<String> {
        let caps: Vec<_> = self.offsets.iter().map(capabilities).collect();
        digest(&(
            "oem-gpu-turbo-1",
            &self.devices,
            self.source.capabilities,
            self.source.limits,
            self.source.saved,
            self.base_mhz,
            caps,
        ))
    }

    pub fn target(&self, event: u8, mode: u8, on_ac: Option<bool>) -> Result<Target> {
        gpu_turbo::target(
            event,
            mode,
            on_ac,
            self.source.capabilities,
            self.source.limits,
            self.source.saved,
            self.devices.len(),
            self.base_mhz,
        )
        .map_err(anyhow::Error::msg)
    }

    pub(super) fn device_readings(&self) -> Vec<DeviceReading> {
        self.devices
            .iter()
            .zip(&self.offsets)
            .map(|(id, p)| DeviceReading {
                id: id.clone(),
                offsets: raw(p),
                core_editable: p.core.editable,
                memory_editable: p.memory.editable,
                core_range_khz: [p.core.min_khz, p.core.max_khz],
                memory_range_khz: [p.memory.min_khz, p.memory.max_khz],
            })
            .collect()
    }
    pub fn view(&self) -> Result<Current> {
        let devices = self.device_readings();
        let mut on_targets = Vec::new();
        for mode in 0..ccc_core::oem_events::POWER_MODE_COUNT {
            for on_ac in [true, false] {
                let mut candidate = self.clone();
                candidate.source.mode = mode;
                candidate.source.on_ac = Some(on_ac);
                on_targets.push(ModeTarget {
                    mode,
                    on_ac,
                    target: self.target(97, mode, Some(on_ac))?,
                    unavailable_reason: super::transaction::validate_plan(&candidate, 97)
                        .err()
                        .map(|error| format!("{error:#}")),
                });
            }
        }
        Ok(Current {
            source: self.source.clone(),
            devices,
            other_devices: self.devices.iter().skip(2).cloned().collect(),
            locks: self.locks.owned()?,
            base_mhz: self.base_mhz,
            off: self.target(96, self.source.mode, self.source.on_ac)?,
            off_error: super::transaction::validate_plan(self, 96)
                .err()
                .map(|error| format!("{error:#}")),
            current_on: self.target(97, self.source.mode, self.source.on_ac).ok(),
            on_targets,
        })
    }
}

pub fn observe_restore(io: &dyn TurboIo, initial: &Initial) -> Result<RestoreReading> {
    if io.devices()? != initial.devices {
        bail!("GPU 设备集合或顺序变化，不能恢复旧设备的初始状态");
    }
    if io.primary()? != initial.devices[0] {
        bail!("GPU 控制会话已变化，不能恢复旧设备的初始状态");
    }
    let locks = if initial.locks.is_some() {
        Some(io.locks(&initial.devices[0])?.owned()?)
    } else {
        None
    };
    let mut offsets = Vec::new();
    for (id, owned) in initial.devices.iter().zip(&initial.offsets) {
        let current = if owned.core_khz.is_some() || owned.mem_khz.is_some() {
            let parsed = io.offsets(id)?;
            if !parsed.p0_present {
                bail!("GPU P0 不可读，无法恢复偏移");
            }
            let current = raw(&parsed);
            if owned.core_khz.is_some() && current.core_khz.is_none()
                || owned.mem_khz.is_some() && current.mem_khz.is_none()
            {
                bail!("GPU 已接管的偏移域不可读");
            }
            current
        } else {
            RawOffsets::default()
        };
        offsets.push(current);
    }
    Ok(RestoreReading { locks, offsets })
}
