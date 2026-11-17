//! 档案显示目标、预览与用户保留后的本次恢复资格。
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub displays: Vec<Mode>,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        if self.displays.is_empty() || self.displays.len() > 32 {
            bail!("显示档案须包含1–32个显示器");
        }
        let mut devices = std::collections::BTreeSet::new();
        for mode in &self.displays {
            mode.validate()?;
            if !devices.insert(&mode.device) {
                bail!("显示档案不能重复配置同一显示器");
            }
        }
        if serde_json::to_vec(self)?.len() > 60 * 1024 {
            bail!("显示档案过大");
        }
        Ok(())
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub displays: Vec<guardian::Change>,
}
impl Review {
    pub fn changes(&self) -> Vec<guardian::Change> {
        self.displays
            .iter()
            .filter(|change| !change.before.same_state(&change.target))
            .cloned()
            .collect()
    }
    fn validate(&self) -> Result<()> {
        Target {
            displays: self
                .displays
                .iter()
                .map(|change| change.target.clone())
                .collect(),
        }
        .validate()?;
        for change in &self.displays {
            change.before.validate()?;
            let mut target = change.target.clone();
            target.frequency = change.before.frequency;
            if !target.same_state(&change.before) {
                bail!("显示档案只允许修改刷新率");
            }
        }
        Ok(())
    }
}

impl Controller {
    pub fn capture(&self) -> Result<Option<Target>> {
        let state = self.lock()?;
        self.ensure_idle(&state)?;
        let displays = self
            .io
            .list()?
            .into_iter()
            .map(|display| display.current)
            .collect::<Vec<_>>();
        if displays.is_empty() {
            return Ok(None);
        }
        let target = Target { displays };
        target.validate()?;
        Ok(Some(target))
    }
    pub fn preview_profile(&self, target: &Target, owner: Option<u64>) -> Result<Review> {
        target.validate()?;
        let state = self.lock()?;
        if let Some(id) = owner {
            self.check_retained(&state, id)?;
        } else {
            self.ensure_idle(&state)?;
        }
        let actual = self.io.list()?;
        let mut displays = Vec::new();
        for saved in &target.displays {
            let display = actual
                .iter()
                .find(|display| display.current.device == saved.device)
                .ok_or_else(|| anyhow!("档案显示器已断开：{}", saved.name))?;
            let mut normalized = saved.clone();
            normalized.frequency = display.current.frequency;
            if !normalized.same_state(&display.current) {
                bail!("档案显示器身份或几何已变化：{}", saved.name);
            }
            if !display.frequencies.contains(&saved.frequency) {
                bail!("档案刷新率已不受支持：{}", saved.name);
            }
            let mut target = display.current.clone();
            target.frequency = saved.frequency;
            displays.push(guardian::Change {
                before: display.current.clone(),
                target,
            });
        }
        let review = Review { displays };
        review.validate()?;
        Ok(review)
    }
    pub fn begin_profile(&self, review: &Review, id: u64, now: u64) -> Result<bool> {
        review.validate()?;
        if id == 0 {
            bail!("显示档案会话标识无效");
        }
        let mut state = self.lock()?;
        self.ensure_idle(&state)?;
        self.check_changes(&review.displays)?;
        let changes = review.changes();
        if changes.is_empty() {
            return Ok(false);
        }
        self.start(&mut state, changes, Owner::Profile(id), now)?;
        Ok(true)
    }
    pub fn keep_profile(&self, id: u64, now: u64) -> Result<()> {
        let mut state = self.lock()?;
        if let Some(error) = &state.error {
            bail!("显示恢复状态存在错误：{error}；请先恢复本次试用");
        }
        let trial = state
            .trial
            .as_mut()
            .filter(|trial| trial.owner == Owner::Profile(id))
            .ok_or_else(|| anyhow!("档案显示试用已结束或不属于此请求"))?;
        self.keep_trial(trial, now)?;
        let trial = state.trial.take().expect("已保留档案试用");
        state.retained = Some(Retained {
            id,
            changes: trial.plan.changes,
        });
        state.error = None;
        Ok(())
    }
    fn check_retained(&self, state: &State, id: u64) -> Result<()> {
        let changes = state
            .retained
            .as_ref()
            .filter(|retained| retained.id == id)
            .map(|retained| &retained.changes)
            .or_else(|| {
                state
                    .trial
                    .as_ref()
                    .filter(|trial| trial.owner == Owner::Profile(id))
                    .map(|trial| &trial.plan.changes)
            })
            .ok_or_else(|| anyhow!("档案显示会话已变化"))?;
        let targets = changes
            .iter()
            .map(|change| change.target.clone())
            .collect::<Vec<_>>();
        if !guardian::matches(self.io.as_ref(), &targets)? {
            bail!("已保留的档案显示模式已变化，停止其余档案输出");
        }
        Ok(())
    }
    pub fn release_profile(&self, id: u64) -> Result<()> {
        let mut state = self.lock()?;
        if state.retained.as_ref().map(|retained| retained.id) != Some(id) {
            bail!("档案显示保留状态已变化");
        }
        state.retained = None;
        state.error = None;
        Ok(())
    }
    pub fn require_kept(&self, id: u64) -> Result<()> {
        let state = self.lock()?;
        if state.retained.as_ref().map(|retained| retained.id) != Some(id) {
            bail!("档案显示尚未明确保留，未执行其余设置");
        }
        self.check_retained(&state, id)
    }
    pub fn cancel_profile(&self, id: u64) -> Result<()> {
        let mut state = self.lock()?;
        let result = if let Some(trial) = state
            .trial
            .as_mut()
            .filter(|trial| trial.owner == Owner::Profile(id))
        {
            self.recover_trial(trial)
        } else if let Some(retained) = state.retained.as_ref().filter(|retained| retained.id == id)
        {
            guardian::restore(self.io.as_ref(), &retained.changes)
        } else {
            bail!("档案显示会话已结束或不属于此请求");
        };
        if let Err(error) = result {
            let message = format!("档案显示恢复失败：{error:#}");
            state.error = Some(message.clone());
            bail!(message);
        }
        state.trial = None;
        state.retained = None;
        state.error = None;
        Ok(())
    }
    pub fn profile_pending(&self, id: u64, now: u64) -> Result<Option<Pending>> {
        Ok(self
            .lock()?
            .pending(now)
            .filter(|pending| pending.profile_session == Some(id)))
    }
}
