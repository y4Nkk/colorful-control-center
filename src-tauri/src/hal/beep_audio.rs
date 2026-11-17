use serde::{Deserialize, Serialize};

#[cfg(not(test))]
mod native;
mod topology;
mod wave;
#[cfg(not(test))]
pub(crate) use native::{capture, set_muted, silence};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Control {
    pub endpoint: String,
    pub volume_part: String,
    pub mute_part: String,
    pub muted: bool,
}

impl Control {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            [&self.endpoint, &self.volume_part, &self.mute_part]
                .into_iter()
                .all(|id| !id.is_empty() && id.len() <= 4096 && !id.chars().any(char::is_control)),
            "原厂PC Beep控件身份无效"
        );
        Ok(())
    }

    pub(crate) fn same_device(&self, other: &Self) -> bool {
        self.endpoint == other.endpoint
            && self.volume_part == other.volume_part
            && self.mute_part == other.mute_part
    }
}
