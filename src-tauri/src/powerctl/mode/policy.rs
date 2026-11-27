use super::*;
use crate::fanctl::{
    self, FAN_MODE_AUTO, FAN_MODE_CUSTOM, FAN_MODE_MAXQ, FAN_MODE_NOISELESS, ModePolicy,
};
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
    pub(crate) fan: ModePolicy,
    pub aspm: Option<[u8; 2]>,
    pub power_plan: Option<u8>,
}
impl Target {
    pub(super) fn command(&self) -> u32 {
        mode_command(self.mode, [u8::from(self.turbo), u8::from(self.dtt)])
    }
}

pub(super) fn target(io: &impl ModeIo, mode: u8, energy: energy::State) -> Result<Target> {
    let mut target = target_for(io, mode, true, Some(energy), read_option_preferences(io)?)?;
    target.ac_mode = Some(mode);
    Ok(target)
}
pub(super) fn target_for(
    io: &impl ModeIo,
    mode: u8,
    activate_plan: bool,
    energy: Option<energy::State>,
    selected_options: [u8; 2],
) -> Result<Target> {
    super::support::ensure(io)?;
    if !available(io)?.contains(&mode) {
        bail!("固件未声明支持此电源模式");
    }
    let caps = io.read(7, 0, 22).map_err(|e| anyhow!(e))?;
    if caps.len() != 22 {
        bail!("能力页长度不足");
    }
    let [turbo_supported, dtt_supported] = super::boost::runtime_options(io, mode)?;
    let turbo = turbo_supported && selected_options[0] == 1;
    let fan = fan_policy(io, mode, &caps, turbo)?;
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
        dtt: dtt_supported && selected_options[1] == 1,
        fan,
        aspm,
        power_plan,
    })
}

fn fan_policy(io: &impl ModeIo, mode: u8, caps: &[u8], turbo: bool) -> Result<ModePolicy> {
    let remembered = fanctl::saved::remembered_mode(io, caps, mode)?;
    let offset_pct = (remembered.is_some() && fanctl::offset_supported(io, caps)?)
        .then(|| byte(io, 4, 74 + fanctl::saved::mode_slot(mode)?))
        .transpose()?;
    let fan = if turbo
        && (mode == 3
            || remembered.is_none_or(|saved| matches!(saved, FAN_MODE_NOISELESS | FAN_MODE_CUSTOM)))
    {
        ModePolicy::explicit(FAN_MODE_AUTO, FAN_MODE_AUTO, offset_pct)
    } else if mode == 3
        && !turbo
        && remembered.is_none_or(|saved| saved == FAN_MODE_AUTO)
        && fanctl::supports_noiseless(caps)
        && fanctl::saved::entertainment_quiet(io)?
    {
        let saved = match remembered {
            Some(saved) => saved,
            None => default_fan_mode(io, mode)?,
        };
        ModePolicy::explicit(FAN_MODE_NOISELESS, saved, offset_pct)
    } else if let Some(saved) = remembered {
        ModePolicy::explicit(saved, saved, offset_pct)
    } else {
        ModePolicy::Firmware {
            saved_mode: default_fan_mode(io, mode)?,
        }
    };
    fan.validate()?;
    Ok(fan)
}

fn default_fan_mode(io: &impl ModeIo, mode: u8) -> Result<u8> {
    if mode == 3 && byte(io, 4, 4)? == FAN_MODE_MAXQ {
        return Ok(FAN_MODE_MAXQ);
    }
    Ok(FAN_MODE_AUTO)
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
        let at = match mode {
            2 => 1,
            3 => 3,
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
            0 => 3,
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
