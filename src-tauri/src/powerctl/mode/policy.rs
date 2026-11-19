use super::*;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub mode: u8,
    #[serde(deserialize_with = "Option::deserialize")]
    pub from_mode: Option<u8>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub gpu: Option<super::links::Review>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub revision: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub unavailable_reason: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub ac_mode: Option<u8>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub energy: Option<EnergyState>,
    pub turbo: bool,
    pub dtt: bool,
    pub fan_mode: u8,
    pub fan_saved_mode: u8,
    pub fan_offset_pct: u8,
    pub aspm: Option<[u8; 2]>,
    pub power_plan: Option<u8>,
}
impl Target {
    pub(super) fn command(&self) -> u32 {
        mode_command(self.mode, [u8::from(self.turbo), u8::from(self.dtt)])
    }
}

pub(super) fn target(io: &impl ModeIo, mode: u8, energy: energy::State) -> Result<Target> {
    let mut target = target_for(io, mode, true, Some(energy))?;
    target.ac_mode = Some(mode);
    Ok(target)
}
pub(super) fn target_for(
    io: &impl ModeIo,
    mode: u8,
    activate_plan: bool,
    energy: Option<energy::State>,
) -> Result<Target> {
    super::support::ensure(io)?;
    if !available(io)?.contains(&mode) {
        bail!("固件未声明支持此电源模式");
    }
    let caps = io.read(7, 0, 22).map_err(|e| anyhow!(e))?;
    if caps.len() != 22 {
        bail!("能力页长度不足");
    }
    let info = io.fan_info()?;
    let keep_fan = crate::fanctl::saved::keeps_mode(&caps);
    let slot = if mode == 4 { 0 } else { mode as usize };
    let [turbo_supported, dtt_supported] = super::boost::runtime_options(io, mode)?;
    let turbo = turbo_supported && byte(io, 4, 8)? == 1;
    let mut fan_mode = crate::fanctl::saved::remembered_mode(io, &caps, mode)?
        .unwrap_or(if mode == 3 && info[14] == 5 { 5 } else { 0 });
    let fan_offset_pct = if keep_fan {
        byte(io, 4, 74 + slot)?
    } else {
        byte(io, 4, 7)?
    };
    if turbo && (mode == 3 || fan_mode == crate::fanctl::FAN_MODE_NOISELESS) {
        fan_mode = crate::fanctl::FAN_MODE_AUTO;
    }
    let fan_saved_mode = fan_mode;
    if mode == 3
        && !turbo
        && fan_mode == 0
        && crate::fanctl::supports_noiseless(&caps)
        && crate::fanctl::saved::entertainment_quiet(io)?
    {
        fan_mode = crate::fanctl::FAN_MODE_NOISELESS;
    }
    crate::fanctl::validate_mode(fan_mode)?;
    if fan_offset_pct > 100 {
        bail!("原厂分模式风扇偏移超出范围");
    }
    let (aspm, power_plan) = if energy.is_some() || turbo_supported || dtt_supported {
        system_policy(io, mode, &caps, activate_plan)?
    } else {
        (None, None)
    };
    Ok(Target {
        mode,
        from_mode: None,
        gpu: None,
        revision: None,
        unavailable_reason: None,
        ac_mode: None,
        energy: energy
            .map(|state| super::energy::desired(mode, state))
            .transpose()?,
        turbo,
        dtt: dtt_supported && byte(io, 1, 32)? == 1,
        fan_mode,
        fan_saved_mode,
        fan_offset_pct,
        aspm,
        power_plan,
    })
}

fn system_policy(
    io: &impl ModeIo,
    mode: u8,
    caps: &[u8],
    activate_plan: bool,
) -> Result<(Option<[u8; 2]>, Option<u8>)> {
    if caps[..2] != [1, 0] {
        return Ok((None, None));
    }
    let aspm = if caps[16] & 8 != 0 {
        let data = io.table(15)?;
        // OnChangePowerMode 的 Quiet Plus 复用娱乐 ASPM，而电源计划复用安静槽。
        let at = match mode {
            2 => 1,
            3 | 4 => 3,
            0 => 5,
            1 => 7,
            _ => unreachable!(),
        };
        let values = [data[at], data[at + 1]];
        if values.iter().any(|v| *v > 2) {
            bail!("原厂链路节能策略不在有效范围 0–2 内");
        }
        Some(values)
    } else if caps[11] & 2 != 0 {
        let value = if mode == 2 { 0 } else { 2 };
        Some([value; 2])
    } else {
        None
    };
    let power_plan = if activate_plan && caps[19] & 2 != 0 {
        let data = io.table(29)?;
        let at = match mode {
            2 => 1,
            3 => 2,
            0 | 4 => 3,
            1 => 4,
            _ => unreachable!(),
        };
        if data[at] > 2 {
            bail!("原厂电源计划索引无效");
        }
        Some(data[at])
    } else {
        None
    };
    Ok((aspm, power_plan))
}
