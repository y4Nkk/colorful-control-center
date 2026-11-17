//! OnChangePowerMode的调校资格；目标仍由97号事件规则唯一计算。
use super::{Capabilities, Limits, Offsets, Target, target};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    pub capabilities: Option<Capabilities>,
    pub limits: Limits,
    pub saved: [Offsets; 2],
    pub gpu_oc: Option<bool>,
    pub simple_gpu_oc: bool,
    pub ready: bool,
    pub gpu_count: u8,
    pub cached_device_id: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    Resume,
    QuietPlus,
    NotReady,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Decision {
    pub target: Option<Target>,
    pub skipped: Option<SkipReason>,
}

impl Policy {
    pub fn from_packet(
        packet: &[u8; 256],
        wmi16: Option<i32>,
        wmi96: Option<i32>,
        product: u32,
        customer: u8,
        cache: &[u8; 13],
        limits: Limits,
        saved: [Offsets; 2],
    ) -> Result<Self, String> {
        let legacy = Capabilities::needs_wmi96(packet)?;
        let flags = if legacy {
            wmi16
        } else {
            Some(u16::from_le_bytes([packet[13], packet[14]]) as i32)
        };
        Ok(Self {
            capabilities: Capabilities::from_packet(packet, wmi96).ok(),
            limits,
            saved,
            gpu_oc: flags.map(|value| value & 0x20 != 0),
            simple_gpu_oc: packet[..2] == [1, 0] && packet[20] & 8 != 0
                || simple_model(product, customer),
            ready: cache[9] != 0,
            gpu_count: cache[6],
            cached_device_id: u16::from_le_bytes([cache[0], cache[1]]),
        })
    }

    pub fn supported(&self) -> Result<bool, String> {
        if self
            .capabilities
            .is_some_and(|caps| caps.limit_clock || caps.all_mode_oc)
            || self.simple_gpu_oc
            || self.gpu_oc == Some(true)
        {
            return Ok(true);
        }
        if self.capabilities.is_none() {
            return Err("原厂 WMI 96 GPU 全模式超频能力不可读".into());
        }
        self.gpu_oc
            .ok_or_else(|| "原厂 WMI 16 GPU 超频能力不可读".into())
    }

    pub fn decide(&self, mode: u8, on_ac: Option<bool>, resume: bool) -> Result<Decision, String> {
        if mode > 4 {
            return Err("未知原厂 GPU 模式".into());
        }
        let skipped = if resume {
            Some(SkipReason::Resume)
        } else if mode == 4 {
            Some(SkipReason::QuietPlus)
        } else if !self.ready {
            Some(SkipReason::NotReady)
        } else if !self.supported()? {
            Some(SkipReason::Unsupported)
        } else {
            None
        };
        if skipped.is_some() {
            return Ok(Decision {
                target: None,
                skipped,
            });
        }
        if self.gpu_count == 0 || self.gpu_count > 64 {
            return Err("原厂 NVIDIA 就绪标志与显卡数量不一致".into());
        }
        let capabilities = self
            .capabilities
            .ok_or("原厂 WMI 96 GPU 全模式超频能力不可读")?;
        Ok(Decision {
            target: Some(target(
                97,
                mode,
                on_ac,
                capabilities,
                self.limits,
                self.saved,
                self.gpu_count.into(),
                None,
            )?),
            skipped: None,
        })
    }
}

fn simple_model(product: u32, customer: u8) -> bool {
    customer != 5
        && matches!(
            product,
            343 | 344
                | 375
                | 376
                | 14081
                | 14082
                | 26273
                | 26274
                | 26275
                | 42576
                | 42577
                | 42593
                | 42609
                | 42610
                | 54529
                | 54530
                | 55041
                | 55042
        )
}

#[cfg(test)]
mod tests;
