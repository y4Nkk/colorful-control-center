use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    pub keyboard_type: u8,
    pub light_bar_switchable: bool,
    pub light_bar_enabled: bool,
    pub cover: bool,
}
impl Capabilities {
    pub fn validate(&self) -> Result<()> {
        if self.keyboard_type != 23 || !self.light_bar_switchable && !self.light_bar_enabled {
            bail!("未知的原厂多区键盘能力");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Values {
    pub enabled: bool,
    pub brightness: u8,
    pub mode: u8,
    pub speed: u8,
    pub zones: [Rgb; 4],
    pub light_bar: Option<Rgb>,
    pub cover: Option<Rgb>,
}
impl Values {
    pub fn validate(&self) -> Result<()> {
        if self.mode > 7 || !(1..=3).contains(&self.speed) || !(1..=4).contains(&self.brightness) {
            bail!("多区键盘须使用原厂8种模式、1–3级速度及1–4级亮度");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub capabilities: Capabilities,
    pub values: Values,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        self.capabilities.validate()?;
        self.values.validate()?;
        if self.values.light_bar.is_some() != self.capabilities.light_bar_enabled
            || self.values.cover.is_some() != self.capabilities.cover
        {
            bail!("多区键盘灯区与原厂能力不一致");
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub target: Target,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub revision: String,
    pub capabilities: Capabilities,
    pub current: Option<Values>,
    pub unavailable_reason: Option<String>,
    pub blocked_reason: Option<String>,
    pub initial: Option<Values>,
    pub can_restore: bool,
}
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub capabilities: Capabilities,
    pub page: Vec<u8>,
}
impl Snapshot {
    fn fields(&self) -> Vec<(usize, usize)> {
        let mut fields = vec![
            (32, 1),
            (34, 1),
            (35, 1),
            (80, 1),
            (85, 3),
            (89, 3),
            (93, 3),
            (10, 3),
        ];
        if self.capabilities.light_bar_enabled {
            fields.push((27, 3));
        }
        if self.capabilities.cover {
            fields.push((13, 3));
        }
        fields
    }
    pub fn validate(&self) -> Result<()> {
        self.capabilities.validate()?;
        if self.page.len() != 256 {
            bail!("多区键盘设置页长度无效");
        }
        Ok(())
    }
    pub fn revision(&self) -> String {
        let values: Vec<_> = self
            .fields()
            .into_iter()
            .flat_map(|(at, len)| &self.page[at..at + len])
            .copied()
            .collect();
        format!(
            "{:x}",
            Sha256::digest(format!("{:?}:{values:?}", self.capabilities))
        )
    }
    pub fn require_device(&self, before: &Self) -> Result<()> {
        self.validate()?;
        if self.capabilities != before.capabilities {
            bail!("多区键盘型号、灯区能力或平台灯条状态已变化");
        }
        Ok(())
    }
    pub fn values(&self) -> Result<Values> {
        self.validate()?;
        if self.page[80] > 1 {
            bail!("多区键盘保存开关未知");
        }
        let color = |at| Rgb {
            r: self.page[at],
            g: self.page[at + 1],
            b: self.page[at + 2],
        };
        let values = Values {
            enabled: self.page[80] == 1,
            brightness: self.page[35],
            mode: self.page[32],
            speed: self.page[34],
            zones: [85, 89, 93, 10].map(color),
            light_bar: self.capabilities.light_bar_enabled.then(|| color(27)),
            cover: self.capabilities.cover.then(|| color(13)),
        };
        values.validate()?;
        Ok(values)
    }
    pub fn target(&self, target: &Target) -> Result<Self> {
        self.validate()?;
        target.validate()?;
        if self.capabilities != target.capabilities {
            bail!("多区键盘目标能力已变化");
        }
        let mut after = self.clone();
        let values = &target.values;
        after.page[32] = values.mode;
        after.page[34] = values.speed;
        after.page[35] = values.brightness;
        after.page[80] = u8::from(values.enabled);
        let colors = [
            Some(values.zones[0]),
            Some(values.zones[1]),
            Some(values.zones[2]),
            Some(values.zones[3]),
            values.light_bar,
            values.cover,
        ];
        for (at, color) in [85, 89, 93, 10, 27, 13].into_iter().zip(colors) {
            if let Some(color) = color {
                after.page[at..at + 3].copy_from_slice(&[color.r, color.g, color.b]);
            }
        }
        Ok(after)
    }
    pub fn patches(&self, before: &Self) -> Result<Vec<Patch>> {
        self.require_device(before)?;
        Ok(self
            .fields()
            .into_iter()
            .filter_map(|(at, len)| {
                (self.page[at..at + len] != before.page[at..at + len])
                    .then(|| Patch::new(at, self.page[at..at + len].to_vec()))
            })
            .collect())
    }
    pub fn packet(&self) -> Result<[u8; 37]> {
        let values = self.values()?;
        let mut packet = [0; 37];
        packet[0] = 44;
        packet[1] = [6, 255, 5, 7, 4, 1, 2, 3][usize::from(values.mode)];
        packet[2] = values.speed;
        let level = usize::from(values.brightness - 1);
        if values.enabled {
            packet[16] = [47, 95, 143, 191][level];
            if values.light_bar.is_some() {
                packet[32] = [31, 46, 87, 128][level];
            }
            if values.cover.is_some() {
                packet[36] = packet[16];
            }
        }
        let inactive = Rgb {
            r: self.page[27],
            g: self.page[28],
            b: self.page[29],
        };
        for (at, color) in [17, 20, 23, 26, 29, 33].into_iter().zip(
            values
                .zones
                .into_iter()
                .map(Some)
                .chain([Some(inactive), values.cover]),
        ) {
            if let Some(color) = color {
                packet[at..at + 3].copy_from_slice(&[color.r, color.g, color.b]);
            }
        }
        Ok(packet)
    }
    pub fn profile(
        &self,
        initial: &Option<Self>,
        blocked_reason: Option<String>,
    ) -> Result<Profile> {
        self.validate()?;
        let current = self.values();
        let can_restore = initial
            .as_ref()
            .is_some_and(|before| self.capabilities == before.capabilities);
        let blocked_reason = blocked_reason.or_else(|| {
            initial
                .as_ref()
                .filter(|before| before.capabilities != self.capabilities)
                .map(|_| {
                    "多区键盘能力或平台灯条状态已变化，初始快照保留；请先核对原设备状态".into()
                })
        });
        Ok(Profile {
            revision: self.revision(),
            capabilities: self.capabilities.clone(),
            unavailable_reason: current.as_ref().err().map(|e| format!("{e:#}")),
            current: current.ok(),
            blocked_reason,
            initial: initial.as_ref().map(Self::values).transpose()?,
            can_restore,
        })
    }
}
