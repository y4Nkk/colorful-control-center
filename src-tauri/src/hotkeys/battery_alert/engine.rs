use super::clock::Clock;
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Binding {
    pub brightness: String,
    pub customer: u8,
    pub low_control: bool,
    pub pc_beep: bool,
    pub codec: Option<String>,
}
impl Binding {
    pub fn identity(&self) -> Result<String> {
        super::super::model::digest(&("battery-alert-1", self))
    }
}
pub(super) trait Io {
    fn binding(&self) -> Result<Binding>;
    fn available(&self) -> Result<()>;
    fn brightness(
        &self,
        binding: &Binding,
        value: u8,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<()>;
    fn low_setting(&self, binding: &Binding, permit: &mut impl FnMut() -> Result<()>)
    -> Result<()>;
    fn mute(
        &self,
        binding: &Binding,
        value: bool,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<()>;
    fn play(&self, enabled: bool) -> Result<()>;
    fn low_bits(&self) -> Result<bool>;
    fn notify(&self) -> Result<()>;
}
#[derive(Default)]
pub(super) struct Engine {
    clock: Clock,
    timer_binding: Option<Binding>,
}
impl Engine {
    pub fn playing(&self) -> bool {
        self.clock.needs_stop()
    }
    pub fn event(
        &mut self,
        io: &impl Io,
        expected: &str,
        active: bool,
        brightness: Option<u8>,
        now: &mut impl FnMut() -> Instant,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        self.clock.ensure_open()?;
        ensure!(active || brightness.is_none(), "结束事件不接受亮度目标");
        ensure!(
            brightness.is_none_or(|value| value <= 100),
            "原厂电池事件亮度超出百分比范围"
        );
        permit()?;
        let binding = io.binding()?;
        ensure!(
            binding.identity()? == expected,
            "电池音频事件来源已变化，请重新确认"
        );
        guard(io, &binding, permit)?;
        if active {
            if let Some(value) = brightness {
                io.brightness(&binding, value, permit)?;
                guard(io, &binding, permit)?;
            }
            if let Some(previous) = &self.timer_binding {
                ensure!(previous == &binding, "待发电池通知来源已变化");
            }
            if self.clock.begin()? {
                self.timer_binding = Some(binding.clone());
                io.play(true)?;
                self.clock.started(now());
            }
            guard(io, &binding, permit)?;
            if binding.pc_beep {
                io.mute(&binding, false, permit)?;
            }
            if binding.low_control {
                guard(io, &binding, permit)?;
                if io.low_bits()? {
                    guard(io, &binding, permit)?;
                    io.low_setting(&binding, permit)?;
                }
            }
        } else {
            if self.clock.needs_stop() {
                io.play(false)?;
                self.clock.stopped();
            }
            guard(io, &binding, permit)?;
            if binding.pc_beep {
                io.mute(&binding, true, permit)?;
            }
        }
        guard(io, &binding, permit)
    }
    pub fn tick(
        &mut self,
        io: &impl Io,
        now: Instant,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<bool> {
        if !self.clock.has_work() {
            return Ok(false);
        }
        let binding = self.timer_binding.clone().context("待发电池通知来源缺失")?;
        guard(io, &binding, permit)?;
        if !self.clock.take_due(now) {
            return Ok(false);
        }
        let low = io.low_bits()?;
        if binding.customer != 5 || !low {
            guard(io, &binding, permit)?;
            io.notify()?;
        }
        Ok(true)
    }
    pub fn cancel(&mut self, io: &impl Io) -> Result<()> {
        self.clock.cancel();
        self.timer_binding = None;
        if self.clock.needs_stop() {
            io.play(false)?;
            self.clock.stopped();
        }
        Ok(())
    }
}
fn guard(io: &impl Io, binding: &Binding, permit: &mut impl FnMut() -> Result<()>) -> Result<()> {
    permit()?;
    io.available()?;
    ensure!(io.binding()? == *binding, "电池音频事件能力或设备已变化");
    Ok(())
}
