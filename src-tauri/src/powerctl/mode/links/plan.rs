use super::{Initial, Session, clocks, settle};
use crate::{gpu_power::mode as node, powerctl::mode::workflow};
use anyhow::{Context, Result, bail};
use ccc_core::{gpu_turbo::mode::SkipReason, oem_supply_mode::Trigger};

pub(crate) struct Effects {
    pub node_before: Option<node::State>,
    pub node_action: Option<node::Action>,
    pub clock_policy: Option<clocks::Policy>,
    pub clock_before: clocks::ClockObservation,
    pub on_ac: bool,
    pub trigger: Option<Trigger>,
}
impl Effects {
    fn validate(&self, mode: u8) -> Result<()> {
        if let Some(state) = &self.node_before {
            state.validate()?;
        }
        if let Some(state) = self.clock_before.raw() {
            state.validate()?;
        }
        node::validate_action(
            self.node_action,
            self.node_before.as_ref(),
            mode,
            self.trigger,
        )?;
        let skipped = if self.trigger == Some(Trigger::Resume) {
            if self.clock_policy.is_some() {
                bail!("唤醒不能携带未使用的GPU调频策略");
            }
            Some(SkipReason::Resume)
        } else {
            self.clock_policy
                .as_ref()
                .context("模式GPU调频策略缺失")?
                .decide(mode, Some(self.on_ac), false)
                .map_err(anyhow::Error::msg)?
                .skipped
        };
        let valid = match (&self.clock_before, skipped) {
            (clocks::ClockObservation::Skipped(actual), Some(expected)) => *actual == expected,
            (clocks::ClockObservation::Ready(_), None) => true,
            (clocks::ClockObservation::PoweredOff, None) => {
                self.node_before.as_ref().is_some_and(|s| !s.target.enabled)
            }
            _ => false,
        };
        if !valid {
            bail!("模式GPU频率前态与目标作用域不一致");
        }
        Ok(())
    }
}
pub(crate) struct Plan {
    pub target: crate::powerctl::mode::Target,
    pub core: workflow::Plan,
    pub effects: Effects,
    pub initial_node: Option<node::State>,
    pub clock_origin: Option<clocks::ClockOrigin>,
}
impl Plan {
    pub fn new(core: workflow::Plan, effects: Effects) -> Result<Self> {
        effects.validate(core.target.mode)?;
        let target = core.target.clone();
        let initial_node = effects.node_before.clone();
        let clock_origin = match &effects.clock_before {
            clocks::ClockObservation::Ready(_) => Some(clocks::ClockOrigin::BeforeMode),
            clocks::ClockObservation::PoweredOff => Some(
                if effects.node_action.is_some_and(|action| {
                    action.stage == node::Stage::BeforeMode && action.target.enabled
                }) {
                    clocks::ClockOrigin::AfterEnable
                } else {
                    clocks::ClockOrigin::PoweredOff
                },
            ),
            _ => None,
        };
        Ok(Self {
            target,
            core,
            effects,
            initial_node,
            clock_origin,
        })
    }
    pub fn review(&self, session: &Session<'_>) -> Result<()> {
        session.review_nodes()?;
        self.core.review(session.context.hal)?;
        self.effects.validate(self.core.target.mode)?;
        if let Some(policy) = &self.effects.clock_policy {
            if session.context.policy()? != *policy {
                bail!("模式GPU策略已变化，未执行模式联动");
            }
        }
        let current = session
            .context
            .device()?
            .as_ref()
            .map(node::State::from_device)
            .transpose()?;
        if current != self.effects.node_before {
            bail!("独显模式前态已变化");
        }
        match &self.effects.clock_before {
            clocks::ClockObservation::Ready(before)
                if session.read_clocks()?.as_ref() != Some(before) =>
            {
                bail!("GPU频率在模式预览后已变化");
            }
            clocks::ClockObservation::PoweredOff
                if current.as_ref().is_none_or(|s| s.target.enabled) =>
            {
                bail!("GPU关断前态已变化");
            }
            _ => (),
        }
        Ok(())
    }
    pub fn execute(
        &self,
        session: &Session<'_>,
        checkpoint: &mut dyn FnMut(&clocks::Write, clocks::ClockOrigin, &str) -> Result<()>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.core.reset_attempted();
        self.review(session)?;
        permit()?;
        let result = (|| {
            self.power(session, node::Stage::BeforeMode, permit)?;
            if let Some(policy) = &self.effects.clock_policy {
                session.prepare_clocks(
                    policy,
                    self.core.target.mode,
                    self.effects.on_ac,
                    false,
                    &self.effects.clock_before,
                    self.clock_origin,
                    checkpoint,
                    permit,
                )?;
            }
            self.core.execute_linked(
                session.context.hal,
                permit,
                &mut |permit| self.after_mode(session, checkpoint, permit),
                &mut |permit| self.power(session, node::Stage::AfterMode, permit),
                &mut || {
                    session.recover_effects(
                        self.effects
                            .node_action
                            .is_some_and(|a| a.stage == node::Stage::AfterMode),
                    )
                },
            )
        })();
        if let Err(error) = result {
            let recovery = session
                .rollback_nodes()
                .map_err(|e| format!("独显恢复：{e:#}"));
            bail!(ccc_core::msr::with_recovery(
                format!("模式联动失败：{error:#}"),
                recovery
            ));
        }
        Ok(())
    }
    fn after_mode(
        &self,
        session: &Session<'_>,
        checkpoint: &mut dyn FnMut(&clocks::Write, clocks::ClockOrigin, &str) -> Result<()>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        if self
            .effects
            .node_action
            .is_some_and(|action| action.stage == node::Stage::BeforeMode)
        {
            Initial::remember_node(
                session.context.hal,
                &self.effects,
                self.initial_node.as_ref(),
            )?;
        }
        settle::wait(
            self.core.target.mode,
            self.effects.node_before.as_ref(),
            permit,
        )?;
        session.apply_clocks(
            &mut || session.context.policy(),
            &mut |write, origin, identity| {
                checkpoint(write, origin, identity)?;
                Initial::remember_clocks(session.context.hal, write, origin, identity)
            },
            permit,
        )
    }
    fn power(
        &self,
        session: &Session<'_>,
        stage: node::Stage,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let Some(action) = self
            .effects
            .node_action
            .filter(|action| action.stage == stage)
        else {
            return Ok(());
        };
        let before = self
            .effects
            .node_before
            .as_ref()
            .context("独显原始状态缺失")?;
        permit()?;
        Initial::remember_node(
            session.context.hal,
            &self.effects,
            self.initial_node.as_ref(),
        )?;
        session.perform(before, action.target, permit)
    }
}
