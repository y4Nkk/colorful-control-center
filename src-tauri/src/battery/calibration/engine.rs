use super::{Access, Baseline, Event, Phase, State, Status, model::commands};
use anyhow::{Context, Result, bail};
use std::time::Duration;

pub(crate) struct Task {
    pub state: State,
    pub baseline: Baseline,
    pub plan_prepared: bool,
    last_tick: Duration,
    ac_acknowledged: bool,
    adjusting_discharge: bool,
    finish_status: Status,
    failure: Option<String>,
}
impl Task {
    pub fn start(
        io: &mut dyn Access,
        baseline: Baseline,
        now: Duration,
        reserve_plan: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Self> {
        baseline.validate()?;
        io.ready()?;
        let facts = io.facts()?;
        facts.before_start()?;
        if facts.stop_threshold != baseline.stop_threshold {
            bail!("校准停止阈值已变化，请重新预览");
        }
        io.verify_baseline(&baseline)?;
        // Drain only before any control write: earlier notifications cannot complete a new stage.
        io.events()?;
        let mut task = Self {
            state: State {
                status: Status::Running,
                phase: Phase::Discharge1,
                elapsed_ms: 0,
                phase_elapsed_ms: 0,
                facts,
                error: None,
            },
            baseline,
            plan_prepared: false,
            last_tick: now,
            ac_acknowledged: false,
            adjusting_discharge: false,
            finish_status: Status::Stopped,
            failure: None,
        };
        if let Err(error) = task.begin(io, reserve_plan) {
            task.fail(io, format!("开始校准失败：{error:#}"));
        }
        Ok(task)
    }
    fn begin(
        &mut self,
        io: &mut dyn Access,
        reserve_plan: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        io.keep_awake(true)?;
        io.prepare_power_plan(&self.baseline.power_scheme)?;
        reserve_plan()?;
        self.plan_prepared = true;
        io.enter_power_plan(&self.baseline.power_scheme)?;
        io.send(121, 0x1c000001)?;
        if self.baseline.energy.is_some() {
            io.send(118, 0x05000000)?;
        }
        self.set_phase(io, Phase::Discharge1)
    }
    fn emit(io: &mut dyn Access, words: [(i32, u32); 3]) -> Result<()> {
        for (command, word) in words {
            io.send(command, word)?;
        }
        Ok(())
    }
    fn set_phase(&mut self, io: &mut dyn Access, phase: Phase) -> Result<()> {
        Self::emit(io, phase.commands())?;
        self.state.phase = phase;
        self.state.phase_elapsed_ms = 0;
        self.ac_acknowledged = false;
        self.adjusting_discharge = false;
        Ok(())
    }
    pub fn tick(&mut self, io: &mut dyn Access, now: Duration) {
        if !self.state.active() {
            return;
        }
        if let Err(error) = self.update(io, now) {
            self.fail(io, format!("校准已中止：{error:#}"));
        }
    }
    fn update(&mut self, io: &mut dyn Access, now: Duration) -> Result<()> {
        let delta = now.checked_sub(self.last_tick).context("校准时钟倒退")?;
        self.last_tick = now;
        if delta > Duration::from_secs(5) {
            bail!("校准调度中断或系统已睡眠，请恢复后重新开始");
        }
        io.ready()?;
        let events = io.events()?;
        io.check_power_plan()?;
        if events.len() > 64 {
            bail!("校准事件积压，无法确认阶段顺序");
        }
        self.state.facts = io.facts()?;
        self.state.facts.validate()?;
        if self.state.facts.stop_threshold != self.baseline.stop_threshold {
            bail!("原厂校准停止阈值已变化，未继续执行旧任务");
        }
        if events.contains(&Event::AcUnplugged) {
            return self.pause(io);
        }
        if self.state.status == Status::Paused {
            if self.state.facts.ac_online == Some(true) {
                self.resume(io)?;
            }
            return Ok(());
        }
        self.advance_clock(delta);
        self.check_ac(io)?;
        if self.state.status == Status::Paused {
            return Ok(());
        }
        if let Some(phase) = self.next_phase(&events) {
            self.set_phase(io, phase)?;
        }
        self.adjust(io)
    }
    fn advance_clock(&mut self, delta: Duration) {
        let ms = delta.as_millis().min(u128::from(u64::MAX)) as u64;
        self.state.elapsed_ms = self.state.elapsed_ms.saturating_add(ms);
        self.state.phase_elapsed_ms = self.state.phase_elapsed_ms.saturating_add(ms);
    }
    fn check_ac(&mut self, io: &mut dyn Access) -> Result<()> {
        if !self.state.phase.charging() {
            return Ok(());
        }
        if self.state.facts.ac_online == Some(true) {
            self.ac_acknowledged = true;
        } else if self.ac_acknowledged {
            self.pause(io)?;
        }
        Ok(())
    }
    fn next_phase(&self, events: &[Event]) -> Option<Phase> {
        let phase = self.state.phase;
        let facts = &self.state.facts;
        let full_status = events.contains(&Event::FullStatus);
        let discharged = events.contains(&Event::FullyDischarged)
            || full_status && facts.battery_status & 0x10 != 0 && facts.ac_online == Some(false);
        if discharged && phase.discharge() {
            return Some(if phase == Phase::Discharge1 {
                Phase::Charge1
            } else {
                Phase::Charge2
            });
        }
        if full_status && facts.battery_status & 0x20 != 0 && phase.charging() {
            return Some(if phase == Phase::Charge1 {
                Phase::Discharge2
            } else {
                Phase::Adjusting
            });
        }
        None
    }
    fn pause(&mut self, io: &mut dyn Access) -> Result<()> {
        if self.state.status != Status::Paused {
            Self::emit(io, commands(true, true, false))?;
            self.state.status = Status::Paused;
        }
        Ok(())
    }
    fn resume(&mut self, io: &mut dyn Access) -> Result<()> {
        Self::emit(io, self.state.phase.commands())?;
        self.ac_acknowledged = self.state.phase.charging();
        self.adjusting_discharge = false;
        self.state.status = Status::Running;
        Ok(())
    }
    fn adjust(&mut self, io: &mut dyn Access) -> Result<()> {
        if self.state.phase != Phase::Adjusting {
            return Ok(());
        }
        let facts = &self.state.facts;
        if facts.percent.context("校准电量未知")? <= facts.stop_threshold {
            self.finish(io, Status::Completed);
        } else if !self.adjusting_discharge {
            Self::emit(io, commands(false, true, false))?;
            self.adjusting_discharge = true;
        }
        Ok(())
    }
    pub fn stop(&mut self, io: &mut dyn Access) {
        if self.state.active() {
            self.finish(io, Status::Stopped);
        }
    }
    pub fn intended_status(&self) -> Status {
        self.finish_status
    }
    pub fn fail(&mut self, io: &mut dyn Access, message: String) {
        self.failure = Some(message.clone());
        self.state.error = Some(message);
        self.finish(io, Status::Failed);
    }
    fn finish(&mut self, io: &mut dyn Access, status: Status) {
        self.finish_status = status;
        self.retry_restore(io);
    }
    pub fn retry_restore(&mut self, io: &mut dyn Access) {
        let errors = release(io, &self.baseline, self.plan_prepared);
        if errors.is_empty() {
            self.state.status = self.finish_status;
            self.state.error.clone_from(&self.failure);
        } else {
            self.state.status = Status::RecoveryRequired;
            let original = self.failure.as_deref().unwrap_or("校准控制待恢复");
            self.state.error = Some(format!("{original}；恢复：{}", errors.join("；")));
        }
    }
}
pub(crate) fn release(
    io: &mut dyn Access,
    baseline: &Baseline,
    plan_prepared: bool,
) -> Vec<String> {
    let mut errors = Vec::new();
    // Each release must run even if an earlier transport operation fails.
    for (command, word) in commands(true, false, true) {
        if let Err(error) = io.send(command, word) {
            errors.push(format!("cmd{command}/{word:#x}：{error:#}"));
        }
    }
    if let Err(error) = io.restore_policy(baseline) {
        errors.push(format!("充电与节能策略：{error:#}"));
    }
    if plan_prepared {
        if let Err(error) = io.restore_power_plan(&baseline.power_scheme) {
            errors.push(format!("校准电源计划：{error:#}"));
        }
    }
    if let Err(error) = io.keep_awake(false) {
        errors.push(format!("释放防睡眠请求：{error:#}"));
    }
    errors
}
