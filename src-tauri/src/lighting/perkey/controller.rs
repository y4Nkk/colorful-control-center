use super::*;
#[cfg(test)]
use std::sync::Arc;

#[derive(Default)]
pub(super) struct State {
    pub baseline: Option<(String, Snapshot)>,
    pub session: Option<runtime::Session>,
    pub error: Option<String>,
}
#[derive(Default)]
pub struct Controller {
    pub(super) state: Mutex<State>,
    #[cfg(test)]
    test_io: Option<Arc<dyn Io + Send + Sync>>,
    #[cfg(test)]
    test_runtime: Option<Arc<runtime::TestRuntime>>,
}
impl Controller {
    pub(crate) fn release_on_exit(&self) -> Result<()> {
        let session = self
            .state
            .lock()
            .map_err(|_| anyhow!("逐键灯效状态异常"))?
            .session
            .take();
        drop(session);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn with_test_io(mut self, io: Arc<dyn Io + Send + Sync>) -> Self {
        self.test_io = Some(io);
        self
    }
    #[cfg(test)]
    pub(crate) fn with_test_runtime(mut self, runtime: Arc<runtime::TestRuntime>) -> Self {
        self.test_runtime = Some(runtime);
        self
    }
    pub(super) fn with_io<T>(
        &self,
        hal: &Hal,
        action: impl FnOnce(u8, &dyn Io) -> Result<T>,
    ) -> Result<T> {
        #[cfg(test)]
        {
            let raw = detected(hal)?.ok_or_else(|| anyhow!("当前设备未确认是逐键键盘"))?;
            return match &self.test_io {
                Some(io) => action(raw, io.as_ref()),
                None => anyhow::bail!("隔离测试未注入逐键传输，未访问真实HID"),
            };
        }
        #[cfg(not(test))]
        {
            let (raw, io) = native(hal)?;
            action(raw, &io)
        }
    }
    pub(super) fn prepare_session(
        &self,
        hal: &Hal,
        target: &Target,
        identity: String,
    ) -> Result<Option<runtime::Session>> {
        if !target.values.enabled || !target.values.pattern.reactive() {
            return Ok(None);
        }
        #[cfg(test)]
        {
            let runtime = self
                .test_runtime
                .as_ref()
                .ok_or_else(|| anyhow!("隔离测试未注入逐键事件传输，未启动真实钩子"))?;
            let io = self
                .test_io
                .as_ref()
                .ok_or_else(|| anyhow!("隔离测试未注入逐键传输"))?;
            runtime.prepare(
                &target.values,
                &target.device,
                identity,
                hal.control_gate(),
                io.clone(),
            )
        }
        #[cfg(not(test))]
        runtime::Session::prepare(&target.values, &target.device, identity, hal.control_gate())
    }
    pub(crate) fn hotkey_identity(&self, hal: &Hal) -> Result<String> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.identity(hal)
    }
    pub(crate) fn toggle_enabled(&self, hal: &Hal, expected: &str) -> Result<bool> {
        let request = {
            let mut scope = self.scope()?;
            let _gate = hal.begin_write()?;
            scope.hotkey_toggle_request(hal, expected)?
        };
        self.apply(hal, &request)?;
        Ok(request.values.enabled)
    }
    pub(crate) fn adjust_brightness(&self, hal: &Hal, expected: &str, delta: i8) -> Result<u8> {
        let request = {
            let mut scope = self.scope()?;
            let _gate = hal.begin_write()?;
            scope.hotkey_request(hal, expected, delta)?
        };
        self.apply(hal, &request)?;
        Ok(request.values.brightness)
    }
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let mut scope = self.scope()?;
        let _write = hal.begin_write()?;
        scope.profile(hal)
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        let mut scope = self.scope()?;
        let plan = {
            let _gate = hal.begin_write()?;
            scope.reviewed(hal, request)?
        };
        scope.run_single(hal, &plan)
    }
    pub fn restore(&self, hal: &Hal) -> Result<()> {
        let mut scope = self.scope()?;
        let plan = {
            let _gate = hal.begin_write()?;
            scope.initial(hal)?
        };
        scope.run_single(hal, &plan)?;
        scope.state.baseline = None;
        Ok(())
    }
}
