//! 模式联动的GPU执行上下文；供电、明确选择及档案共用节点与频率事务。
use crate::{
    gpu_power,
    gpuctl::{GpuController, turbo::mode as clocks},
    hal::Hal,
};
use anyhow::{Result, bail};
use std::cell::{Cell, RefCell};
#[path = "links/source.rs"]
mod source;
pub(crate) use source::Source;
#[path = "links/clocks.rs"]
mod frequency;
#[path = "links/plan.rs"]
mod plan;
pub(crate) use plan::{Effects, Plan};
#[path = "links/nodes.rs"]
mod nodes;
#[path = "links/selection.rs"]
mod selection;
pub(crate) use selection::Review;
#[cfg(test)]
pub(crate) use selection::Target;
#[path = "links/initial.rs"]
mod initial;
pub(crate) use initial::Initial;

pub(crate) struct Context<'a> {
    pub hal: &'a Hal,
    pub clocks: &'a GpuController,
    power: &'a gpu_power::Controller,
    source: Box<dyn Source + 'a>,
}
impl<'a> Context<'a> {
    pub fn native(
        hal: &'a Hal,
        clocks: &'a GpuController,
        power: &'a gpu_power::Controller,
    ) -> Self {
        Self {
            hal,
            clocks,
            power,
            source: Box::new(source::Native(hal)),
        }
    }
    #[cfg(test)]
    pub(crate) fn with_source(
        hal: &'a Hal,
        clocks: &'a GpuController,
        power: &'a gpu_power::Controller,
        source: Box<dyn Source + 'a>,
    ) -> Self {
        Self {
            hal,
            clocks,
            power,
            source,
        }
    }
    pub fn devices(&self) -> Result<Vec<gpu_power::Device>> {
        self.power.list_in_control(self.hal)
    }
    pub fn device(&self) -> Result<Option<gpu_power::Device>> {
        match self.devices()?.as_slice() {
            [] => Ok(None),
            [device] if device.healthy() => Ok(Some(device.clone())),
            _ => bail!("原厂独显节点不唯一或状态异常"),
        }
    }
    pub fn inspect(
        &self,
        review: &mut clocks::Review,
        modes: [Option<u8>; 6],
        on_ac: Option<bool>,
        powered_off: bool,
    ) {
        self.clocks.inspect_mode_gpu_in_control(
            review,
            self.source.as_ref(),
            modes,
            on_ac,
            powered_off,
        );
    }
    pub fn policy(&self) -> Result<clocks::Policy> {
        self.source.policy()
    }
    pub fn inspect_cycle(&self, review: &mut clocks::Cycle, on_ac: Option<bool>, off: bool) {
        self.clocks
            .inspect_mode_cycle_in_control(review, self.source.as_ref(), on_ac, off);
    }
    // 在HAL锁外取得节点互斥；只读Context不持有该互斥。
    pub fn session(self) -> Result<Session<'a>> {
        let gpu = self.power.scope()?;
        Ok(Session {
            context: self,
            gpu: RefCell::new(gpu),
            pending: RefCell::new(None),
            authorized: Cell::new(false),
            clock_prepared: RefCell::new(None),
            clock_prepared_once: Cell::new(false),
            clock_started: Cell::new(false),
        })
    }
}
struct PreparedClocks {
    value: clocks::Prepared,
    origin: clocks::ClockOrigin,
}
pub(crate) struct Session<'a> {
    pub context: Context<'a>,
    gpu: RefCell<gpu_power::Scope<'a>>,
    pending: RefCell<Option<Vec<gpu_power::request::Request>>>,
    authorized: Cell<bool>,
    clock_prepared: RefCell<Option<PreparedClocks>>,
    clock_prepared_once: Cell<bool>,
    clock_started: Cell<bool>,
}
impl Session<'_> {
    pub fn recover_effects(&self, after_mode_node: bool) -> Result<()> {
        let mut errors = Vec::new();
        let node_attempted = after_mode_node && self.node_scope().attempted();
        if node_attempted {
            if let Err(error) = self.rollback_nodes() {
                errors.push(format!("独显恢复：{error:#}"));
            }
        }
        if let Err(error) = self.recover_clocks() {
            errors.push(format!("GPU频率恢复：{error:#}"));
        }
        if !errors.is_empty() {
            bail!(errors.join("；"));
        }
        Ok(())
    }
}
impl<'a> Session<'a> {
    pub(crate) fn node_scope(&self) -> std::cell::RefMut<'_, gpu_power::Scope<'a>> {
        self.gpu.borrow_mut()
    }
}
