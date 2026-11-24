use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Endpoint {
    pub id: String,
    pub name: Option<String>,
    pub muted: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub endpoints: Vec<Endpoint>,
    pub saved: u8,
}
impl Snapshot {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.endpoints.len() > 128 {
            bail!("活动麦克风超过支持数量");
        }
        let mut previous: Option<&str> = None;
        for endpoint in &self.endpoints {
            if endpoint.id.is_empty()
                || endpoint.id.len() > 4096
                || endpoint.id.contains('\0')
                || previous.is_some_and(|id| id >= endpoint.id.as_str())
            {
                bail!("麦克风身份为空、重复或顺序无效");
            }
            if endpoint
                .name
                .as_ref()
                .is_some_and(|name| name.len() > 4096 || name.contains('\0'))
            {
                bail!("麦克风名称无效");
            }
            previous = Some(&endpoint.id);
        }
        Ok(())
    }
    pub(crate) fn same_devices(&self, other: &Self) -> Result<()> {
        if self.endpoints.is_empty()
            || self
                .endpoints
                .iter()
                .map(|e| &e.id)
                .ne(other.endpoints.iter().map(|e| &e.id))
        {
            bail!("活动麦克风集合已变化，请重新预览；不接管新接入的设备");
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Result<String> {
        self.validate()?;
        if self.endpoints.is_empty() {
            bail!("没有活动麦克风");
        }
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(
                &self.endpoints.iter().map(|e| &e.id).collect::<Vec<_>>()
            )?)
        ))
    }
    pub(crate) fn revision(&self) -> Result<String> {
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(self)?)
        ))
    }
    pub(crate) fn target(&self, target: &Target) -> Result<Self> {
        target.validate()?;
        let mut after = self.clone();
        let desired = Snapshot {
            endpoints: target.endpoints.clone(),
            saved: u8::from(target.saved_muted),
        };
        self.same_devices(&desired)?;
        for (current, next) in after.endpoints.iter_mut().zip(&target.endpoints) {
            current.muted = next.muted;
        }
        after.saved = desired.saved;
        Ok(after)
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub endpoints: Vec<Endpoint>,
    pub saved_muted: bool,
}
impl Target {
    pub(crate) fn validate(&self) -> Result<()> {
        let snapshot = Snapshot {
            endpoints: self.endpoints.clone(),
            saved: u8::from(self.saved_muted),
        };
        snapshot.validate()?;
        if snapshot.endpoints.is_empty() {
            bail!("麦克风目标不能为空");
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub muted: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub revision: String,
    pub current: Snapshot,
    pub blocked_reason: Option<String>,
}
