use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub(super) trait Io {
    fn packet(&self) -> Result<[u8; 256]>;
    fn legacy_flags(&self) -> Result<u32>;
}

impl Io for crate::hal::oem_dchu::Dchu {
    fn packet(&self) -> Result<[u8; 256]> {
        self.bios_capabilities()
    }
    fn legacy_flags(&self) -> Result<u32> {
        let (valid, value) = self.read_int(122);
        if !valid {
            bail!("原厂 WMI122 能力不可读");
        }
        Ok(value as u32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    pub hardware_p_state: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub energy_star: Option<u8>,
}

impl State {
    pub(super) fn validate(self) -> Result<()> {
        if self.hardware_p_state != self.energy_star.is_none()
            || self.energy_star.is_some_and(|value| value > 2)
        {
            bail!("EnergyStar 状态记录无效");
        }
        Ok(())
    }
}

pub(super) fn read(io: &impl Io, reviewed_dsdt: bool) -> Result<State> {
    let packet = io.packet()?;
    let flags = match (packet[0], packet[1]) {
        (0, 0) => io.legacy_flags()?,
        (1, 0) => {
            let wmi82 = u32::from_le_bytes(packet[2..6].try_into().unwrap());
            let wmi122 = u32::from_le_bytes(packet[6..10].try_into().unwrap());
            let wmi96 = u16::from_le_bytes(packet[10..12].try_into().unwrap()) as u32;
            let wmi16 = u16::from_le_bytes(packet[13..15].try_into().unwrap()) as u32;
            if wmi82
                .wrapping_add(wmi122)
                .wrapping_add(wmi96)
                .wrapping_add(wmi16)
                == 0
            {
                io.legacy_flags()?
            } else {
                wmi122
            }
        }
        _ => bail!("原厂 EnergyStar 能力包版本未知"),
    };
    if flags & 0x4000_0000 != 0 {
        return Ok(State {
            hardware_p_state: true,
            energy_star: None,
        });
    }
    if !reviewed_dsdt || packet[..2] != [1, 0] {
        bail!("当前固件的 EnergyStar 状态回读未核对，无法确认恢复前态");
    }
    let value = (packet[2] >> 4) & 3;
    if value > 2 {
        bail!("原厂 EnergyStar 当前状态没有已确认的恢复命令");
    }
    Ok(State {
        hardware_p_state: false,
        energy_star: Some(value),
    })
}

pub(super) fn target(mode: u8, state: State) -> Result<Option<u8>> {
    state.validate()?;
    if mode > 4 {
        bail!("未知原厂电源模式");
    }
    if state.hardware_p_state {
        return Ok(None);
    }
    if state.energy_star.is_none() {
        bail!("EnergyStar 状态不可恢复");
    }
    Ok(Some(match mode {
        0 | 3 => 2,
        1 | 4 => 1,
        2 => 0,
        _ => unreachable!(),
    }))
}
pub(super) fn desired(mode: u8, before: State) -> Result<State> {
    Ok(State {
        energy_star: target(mode, before)?,
        ..before
    })
}
pub(super) fn apply(
    hal: &crate::hal::Hal,
    before: Option<State>,
    desired: Option<State>,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let (before, desired) = match (before, desired) {
        (None, None) => return Ok(()),
        (Some(before), Some(desired)) if before.hardware_p_state == desired.hardware_p_state => {
            (before, desired)
        }
        _ => bail!("EnergyStar目标与模式检查点的所有权或能力不一致"),
    };
    desired.validate()?;
    let output = desired.energy_star.filter(|_| desired != before);
    if output.is_some() {
        permit()?;
    }
    hal.with_dchu(|io| {
        if read(io, hal.reviewed_dsdt)? != before {
            bail!("EnergyStar 能力或前态已变化，请重新预览");
        }
        if let Some(value) = output {
            io.set_u32(79, u32::from(value))?;
            if read(io, hal.reviewed_dsdt)? != desired {
                bail!("EnergyStar 命令后状态回读不一致");
            }
        }
        Ok(())
    })
}
pub(super) fn review_restore(
    hal: &crate::hal::Hal,
    initial: Option<State>,
) -> Result<Option<State>> {
    initial
        .map(|initial| {
            hal.with_dchu(|io| {
                let current = read(io, hal.reviewed_dsdt)?;
                if current.hardware_p_state != initial.hardware_p_state {
                    bail!("EnergyStar能力已变化，未恢复其他模式设置");
                }
                Ok(current)
            })
        })
        .transpose()
}
pub(super) fn restore_issue(initial: Option<State>, current: Option<State>) -> Option<String> {
    match (initial, current) {
        (Some(_), None) => Some("初始模式包含固件节能，当前状态未知，暂不能恢复".into()),
        (Some(initial), Some(current)) if initial.hardware_p_state != current.hardware_p_state => {
            Some("固件节能管理能力已变化，不能恢复此初始模式".into())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
