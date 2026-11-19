use super::*;

impl Controller {
    pub fn apply_rotation(&self, expected: &Mode, now: u64) -> Result<()> {
        let mut state = self.lock()?;
        self.ensure_idle(&state)?;
        let change = guardian::Change {
            before: expected.clone(),
            target: expected.oem_rotation()?,
        };
        self.start(&mut state, vec![change], Owner::Manual, now)
    }
    pub fn apply(&self, expected: &Mode, frequency: u32, now: u64) -> Result<()> {
        let mut state = self.lock()?;
        self.ensure_idle(&state)?;
        let mut target = expected.clone();
        target.frequency = frequency;
        let change = guardian::Change {
            before: expected.clone(),
            target,
        };
        self.check_changes(std::slice::from_ref(&change))?;
        if frequency == expected.frequency {
            return Ok(());
        }
        self.start(&mut state, vec![change], Owner::Manual, now)
    }
    pub(super) fn begin_automatic(
        &self,
        expected: &Mode,
        frequency: u32,
        now: u64,
    ) -> Result<Option<u64>> {
        let mut state = self.lock()?;
        self.ensure_idle(&state)?;
        if !primary_mode(&self.io.list()?)?.same_state(expected) {
            bail!("自动刷新率的主显示器已变化");
        }
        let mut target = expected.clone();
        target.frequency = frequency;
        let change = guardian::Change {
            before: expected.clone(),
            target,
        };
        self.check_changes(std::slice::from_ref(&change))?;
        if frequency == expected.frequency {
            return Ok(None);
        }
        self.start(&mut state, vec![change], Owner::Automatic, now)?;
        Ok(Some(state.sequence))
    }
    pub(super) fn automatic_outcome(&self, id: u64) -> Result<Option<AutomaticOutcome>> {
        let state = self.lock()?;
        if state
            .automatic_outcome
            .is_some_and(|outcome| outcome.id != id)
        {
            bail!("自动显示试用结果已变化");
        }
        Ok(state.automatic_outcome)
    }
    pub(super) fn clear_automatic_outcome(&self, expected: AutomaticOutcome) -> Result<()> {
        let mut state = self.lock()?;
        if state.automatic_outcome != Some(expected) {
            bail!("自动显示试用结果已变化");
        }
        state.automatic_outcome = None;
        Ok(())
    }
    pub(super) fn automatic_trial_id(&self) -> Result<Option<u64>> {
        Ok(self
            .lock()?
            .trial
            .as_ref()
            .and_then(|trial| (trial.owner == Owner::Automatic).then_some(trial.id)))
    }
    pub(super) fn automatic_result(&self) -> Result<Option<AutomaticOutcome>> {
        Ok(self.lock()?.automatic_outcome)
    }
    pub(super) fn recover_automatic(&self, change: &guardian::Change) -> Result<()> {
        let state = self.lock()?;
        self.ensure_idle(&state)?;
        let actual = primary_mode(&self.io.list()?)?;
        if !actual.same_state(&change.before) && !actual.same_state(&change.target) {
            bail!("主屏模式已变化，未覆盖外部设置");
        }
        guardian::restore(self.io.as_ref(), std::slice::from_ref(change))
    }
    pub(super) fn ensure_idle(&self, state: &State) -> Result<()> {
        if state.trial.is_some() || state.retained.is_some() || state.automatic_outcome.is_some() {
            bail!("先保留或恢复当前显示试用与档案显示变更");
        }
        Ok(())
    }
    pub(super) fn check_changes(&self, changes: &[guardian::Change]) -> Result<()> {
        let actual = self.io.list()?;
        for change in changes {
            let display = actual
                .iter()
                .find(|display| display.current.device == change.before.device)
                .ok_or_else(|| anyhow!("显示器已断开"))?;
            if !display.current.same_state(&change.before) {
                bail!("显示模式已变化，请重新读取并确认");
            }
            if change.is_rotation()
                && (!display.primary || actual.iter().filter(|item| item.primary).count() != 1)
            {
                bail!("原厂旋转只适用于当前主显示器");
            }
            if !display.frequencies.contains(&change.target.frequency) {
                bail!("当前分辨率不支持此刷新率");
            }
            if !change.before.same_state(&change.target) {
                self.io.test(&change.target)?;
            }
        }
        Ok(())
    }
    pub(super) fn start(
        &self,
        state: &mut State,
        changes: Vec<guardian::Change>,
        owner: Owner,
        now: u64,
    ) -> Result<()> {
        self.ensure_idle(state)?;
        let plan = guardian::Plan {
            changes,
            timeout_ms: 15_000,
        };
        plan.validate()?;
        self.check_changes(&plan.changes)?;
        let id = state
            .sequence
            .checked_add(1)
            .filter(|value| *value <= 9_007_199_254_740_991)
            .ok_or_else(|| anyhow!("显示试用序号耗尽"))?;
        let guard = self.guards.start(plan.clone())?;
        state.sequence = id;
        state.error = None;
        state.trial = Some(Trial {
            id,
            plan,
            deadline: now + 15_000,
            guard,
            owner,
        });
        let result = self.write_trial(state);
        if let Err(error) = result {
            let recovery = self.recover_trial(state.trial.as_mut().expect("已武装监护"));
            if recovery.is_ok() {
                state.trial = None;
            }
            let message = ccc_core::msr::with_recovery(
                format!("显示模式切换失败：{error:#}"),
                recovery.map_err(|error| format!("{error:#}")),
            );
            state.error = Some(message.clone());
            bail!(message);
        }
        Ok(())
    }
    fn write_trial(&self, state: &mut State) -> Result<()> {
        let trial = state.trial.as_mut().expect("已武装监护");
        let started = std::time::Instant::now();
        for change in &trial.plan.changes {
            state
                .baseline
                .entry(change.before.device.clone())
                .or_insert_with(|| change.before.clone());
            self.io.set(&change.target)?;
            if !guardian::matches(self.io.as_ref(), std::slice::from_ref(&change.target))? {
                bail!("显示切换回读与目标不一致");
            }
            if started.elapsed().as_secs() >= 15 || trial.guard.recovered()? {
                bail!("写入期间显示试用已结束，恢复原模式");
            }
        }
        Ok(())
    }
    pub fn keep(&self, id: u64, expected: &Mode, now: u64) -> Result<()> {
        self.keep_owned(id, expected, now, false)
    }
    pub(super) fn keep_automatic(&self, id: u64, expected: &Mode, now: u64) -> Result<()> {
        self.keep_owned(id, expected, now, true)
    }
    fn keep_owned(&self, id: u64, expected: &Mode, now: u64, automatic: bool) -> Result<()> {
        let mut state = self.lock()?;
        if let Some(error) = &state.error {
            bail!("显示恢复状态存在错误：{error}；请先恢复本次试用");
        }
        let trial = state
            .trial
            .as_mut()
            .ok_or_else(|| anyhow!("没有待保留的显示模式"))?;
        if trial.id != id {
            bail!("显示试用已变化，请重新查看");
        }
        if matches!(trial.owner, Owner::Profile(_)) {
            bail!("此试用属于配置档案，请确认完整档案显示变更");
        }
        if (trial.owner == Owner::Automatic) != automatic {
            bail!("显示试用的确认入口已变化");
        }
        if trial.plan.changes.len() != 1 || !trial.plan.changes[0].target.same_state(expected) {
            bail!("试用目标已变化");
        }
        self.keep_trial(trial, now)?;
        let trial = state.trial.take().expect("已保留显示试用");
        record_automatic_outcome(&mut state, &trial, AutomaticResult::Kept);
        state.error = None;
        Ok(())
    }
    pub fn cancel(&self, id: u64) -> Result<()> {
        let mut state = self.lock()?;
        let trial = state
            .trial
            .as_mut()
            .ok_or_else(|| anyhow!("没有待恢复的显示试用"))?;
        if trial.id != id {
            bail!("显示试用已变化，请重新查看");
        }
        if matches!(trial.owner, Owner::Profile(_)) {
            bail!("此试用属于配置档案，请从档案确认中恢复");
        }
        if let Err(error) = self.recover_trial(trial) {
            let message = format!("显示试用恢复失败：{error:#}");
            state.error = Some(message.clone());
            bail!(message);
        }
        let trial = state.trial.take().expect("已恢复显示试用");
        record_automatic_outcome(&mut state, &trial, AutomaticResult::Restored);
        state.error = None;
        Ok(())
    }
    pub(super) fn keep_trial(&self, trial: &mut Trial, now: u64) -> Result<()> {
        if now >= trial.deadline {
            bail!("试用已过期，请恢复后重新预览");
        }
        let targets = trial.plan.targets();
        if !guardian::matches(self.io.as_ref(), &targets)? {
            bail!("实际显示模式与试用目标不一致");
        }
        trial.guard.keep(&targets)
    }
    pub fn tick(&self, now: u64) -> Result<()> {
        let mut state = self.lock()?;
        let Some(mut trial) = state.trial.take() else {
            return Ok(());
        };
        let status = trial.guard.recovered();
        if matches!(status, Ok(true))
            && guardian::matches(self.io.as_ref(), &trial.plan.before()).unwrap_or(false)
        {
            record_automatic_outcome(&mut state, &trial, AutomaticResult::Restored);
            state.error = None;
            return Ok(());
        }
        if matches!(status, Ok(false)) && now < trial.deadline {
            state.trial = Some(trial);
            return Ok(());
        }
        if let Err(error) = self.recover_trial(&mut trial) {
            let message = format!("显示模式自动恢复失败：{error:#}；监护状态：{status:?}");
            state.error = Some(message.clone());
            state.trial = Some(trial);
            bail!(message);
        }
        record_automatic_outcome(&mut state, &trial, AutomaticResult::Restored);
        state.error = status
            .err()
            .map(|error| format!("显示监护异常，已恢复原模式：{error:#}"));
        Ok(())
    }
    pub(super) fn recover_trial(&self, trial: &mut Trial) -> Result<()> {
        guardian::restore(self.io.as_ref(), &trial.plan.changes)?;
        trial.guard.restored(&trial.plan.before())
    }
}
fn record_automatic_outcome(state: &mut State, trial: &Trial, result: AutomaticResult) {
    if trial.owner == Owner::Automatic {
        state.automatic_outcome = Some(AutomaticOutcome {
            id: trial.id,
            result,
        });
    }
}
