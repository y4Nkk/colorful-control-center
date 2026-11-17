use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Encoding {
    LegacyZeroBased,
    LegacyOneBased,
    AdapterV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub level: u8,
    pub encoding: Encoding,
    pub adapter_watts: Option<u8>,
    pub available_levels: Vec<u8>,
}

impl Target {
    pub fn validate(&self) -> Result<()> {
        let levels = &self.available_levels;
        if levels.is_empty()
            || levels.windows(2).any(|pair| pair[0] >= pair[1])
            || !levels.contains(&self.level)
        {
            bail!("PD档案等级须属于有序且不重复的候选");
        }
        let valid = match self.encoding {
            Encoding::LegacyZeroBased => levels == &[0, 1] && self.adapter_watts.is_none(),
            Encoding::LegacyOneBased => {
                levels.iter().copied().eq(1..=*levels.last().unwrap())
                    && self.adapter_watts.is_none()
            }
            Encoding::AdapterV1 => {
                levels.iter().all(|level| (1..=4).contains(level)) && self.adapter_watts != Some(0)
            }
        };
        if !valid {
            bail!("PD档案的编号、适配器与候选不一致");
        }
        Ok(())
    }
}

impl Snapshot {
    pub fn profile_target(&self) -> Target {
        Target {
            level: self.level(),
            encoding: if self.adapter_protocol() {
                Encoding::AdapterV1
            } else if self.zero_based() {
                Encoding::LegacyZeroBased
            } else {
                Encoding::LegacyOneBased
            },
            adapter_watts: self.adapter_watts(),
            available_levels: self.levels.clone(),
        }
    }

    pub fn require_target(&self, target: &Target) -> Result<()> {
        target.validate()?;
        let mut facts = self.profile_target();
        facts.level = target.level;
        if facts != *target {
            bail!("PD档案的编号、适配器或候选与当前设备不一致");
        }
        Ok(())
    }
}
