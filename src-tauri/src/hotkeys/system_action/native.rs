use super::*;

pub(super) struct Native<'a>(pub &'a Hal);
impl Io for Native<'_> {
    fn locked<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let _gate = self.0.begin_write()?;
        f()
    }
    fn snapshot(&self) -> Result<Snapshot> {
        shutdown_identity(self.0)?;
        let plan = self.0.system_power.active()?;
        Snapshot::new(
            plan,
            self.0.system_power.lid_action(plan)?,
            crate::hal::system_action::modern_standby()?,
        )
    }
    fn on_ac(&self) -> Result<bool> {
        #[cfg(test)]
        {
            bail!("隔离测试未注入系统供电状态");
        }
        #[cfg(not(test))]
        {
            crate::battery::system_power()?
                .ac_online
                .ok_or_else(|| anyhow::anyhow!("当前供电状态未知，未请求合盖动作"))
        }
    }
    fn ensure_writable(&self) -> Result<()> {
        shutdown_identity(self.0)?;
        self.0.ensure_mode_change_allowed()?;
        #[cfg(test)]
        {
            bail!("隔离测试未注入系统动作许可");
        }
        #[cfg(not(test))]
        {
            crate::hal::oem_owner::ensure_owner_absent()?;
            crate::hal::oem_owner::ensure_battery_owner_absent()
        }
    }
    fn request(&self, kind: Kind) -> Result<()> {
        crate::hal::system_action::request(kind)
    }
}
pub(super) fn profile(hal: &Hal) -> Profile {
    let io = Native(hal);
    let current = io.locked(|| io.snapshot());
    let identity = current.as_ref().ok().and_then(|s| s.identity().ok());
    let power = io.on_ac();
    Profile {
        identity,
        current: current.as_ref().ok().cloned(),
        on_ac: power.as_ref().ok().copied(),
        power_error: power.err().map(|error| format!("{error:#}")),
        error: current.err().map(|error| format!("{error:#}")),
    }
}
