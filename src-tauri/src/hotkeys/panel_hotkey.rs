mod model;
mod native;
mod resource;
#[cfg(test)]
mod tests;
use anyhow::{Context, Result, anyhow, bail};
use ccc_core::panel_hotkey::{Kind, requests_wake};
pub(crate) use model::Profile;
use model::{Owned, Source};
use resource::{Event, Resource};
use std::sync::{Mutex, MutexGuard};

trait Io {
    fn current(&self) -> Result<Source>;
    fn prepare(&self, source: &Source) -> Result<Box<dyn Resource>>;
    fn wake(&self, source: &Source) -> Result<()>;
}
#[derive(Default)]
struct State {
    next: u64,
    owned: Option<Owned>,
    resource: Option<Box<dyn Resource>>,
    frozen: bool,
}
#[derive(Default)]
pub(super) struct Controller {
    state: Mutex<State>,
    operation: Mutex<()>,
}
pub(super) struct Frozen<'a>(&'a Controller);
impl Drop for Frozen<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state() {
            state.frozen = false;
        }
    }
}
impl Controller {
    fn state(&self) -> Result<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| anyhow!("屏幕按键会话状态异常"))
    }
    pub fn read(&self, hal: &crate::hal::Hal) -> Result<Profile> {
        self.preview(&native::Native(hal))
    }
    fn preview(&self, io: &impl Io) -> Result<Profile> {
        let source = io
            .current()
            .and_then(|source| Ok((source.identity()?, source)));
        let session = self.state()?.owned.clone();
        Ok(Profile {
            restore_revision: session.as_ref().map(Owned::revision).transpose()?,
            session,
            error: source.as_ref().err().map(|e| format!("{e:#}")),
            identity: source.as_ref().ok().map(|(identity, _)| identity.clone()),
            current: source.ok().map(|(_, source)| source),
        })
    }
    pub fn active(&self) -> Result<bool> {
        Ok(self.state()?.owned.is_some())
    }
    pub fn restore_revision(&self) -> Result<Option<String>> {
        self.state()?
            .owned
            .as_ref()
            .map(Owned::revision)
            .transpose()
    }
    pub fn freeze(&self, expected: &str) -> Result<Frozen<'_>> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| anyhow!("屏幕按键执行状态异常"))?;
        let mut state = self.state()?;
        if state.frozen
            || state
                .owned
                .as_ref()
                .map(Owned::revision)
                .transpose()?
                .as_deref()
                != Some(expected)
        {
            bail!("屏幕按键会话预览已变化，请重新查看");
        }
        state.frozen = true;
        Ok(Frozen(self))
    }
    pub fn end(&self, expected: Option<&str>) -> Result<bool> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| anyhow!("屏幕按键执行状态异常"))?;
        if let Some(expected) = expected {
            if self.restore_revision()?.as_deref() != Some(expected) {
                bail!("屏幕按键会话预览已变化，请重新查看");
            }
        }
        let resource = self.state()?.resource.take();
        match resource {
            Some(resource) => {
                self.release(resource)?;
                Ok(true)
            }
            None if self.state()?.owned.is_some() => bail!("屏幕按键会话仍在处理，请稍后重试"),
            None => Ok(false),
        }
    }
    fn release(&self, mut resource: Box<dyn Resource>) -> Result<()> {
        let result = resource.stop();
        let mut state = self.state()?;
        if let Err(error) = result {
            if let Some(owned) = &mut state.owned {
                owned.error = Some(format!("{error:#}"));
            }
            state.resource = Some(resource);
            return Err(error.context("结束本软件屏幕按键会话失败，可重新尝试"));
        }
        state.owned = None;
        Ok(())
    }
    pub fn run(
        &self,
        hal: &crate::hal::Hal,
        identity: &str,
        permit: impl FnMut() -> Result<bool>,
        wait: impl FnMut() -> Result<bool>,
    ) -> Result<Option<String>> {
        self.begin(&native::Native(hal), identity, permit, wait)
    }
    fn begin(
        &self,
        io: &impl Io,
        identity: &str,
        mut permit: impl FnMut() -> Result<bool>,
        mut wait: impl FnMut() -> Result<bool>,
    ) -> Result<Option<String>> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| anyhow!("屏幕按键执行状态异常"))?;
        if self.state()?.frozen {
            return Ok(None);
        }
        if !permit()? {
            return Ok(None);
        }
        let source = io.current()?;
        source.require(identity)?;
        if let Some(owned) = &self.state()?.owned {
            if owned.identity == identity && owned.error.is_none() {
                return Ok(Some("本软件屏幕按键会话已在进行".into()));
            }
            bail!("请先结束已有屏幕按键会话，再重新确认");
        }
        let mut resource = io.prepare(&source)?;
        {
            let mut state = self.state()?;
            state.next = state
                .next
                .checked_add(1)
                .context("屏幕按键会话编号已用尽")?;
            state.owned = Some(Owned {
                id: state.next,
                identity: identity.into(),
                kind: source.kind,
                preparing: true,
                error: None,
            });
        }
        let result = (|| -> Result<bool> {
            if source.kind == Kind::BlankSaver && !wait()? {
                return Ok(false);
            }
            if !permit()? {
                return Ok(false);
            }
            io.current()?.require(identity)?;
            if !permit()? {
                return Ok(false);
            }
            resource.start()?;
            Ok(true)
        })();
        match result {
            Ok(true) => {
                let mut state = self.state()?;
                state.owned.as_mut().unwrap().preparing = false;
                state.resource = Some(resource);
                Ok(Some(
                    match source.kind {
                        Kind::BlankSaver => "已启动本软件空白屏保，未发送显示电源命令",
                        Kind::PointerWake => {
                            "正在等待一次输入：键盘按下结束等待，鼠标活动发送唤醒请求"
                        }
                    }
                    .into(),
                ))
            }
            result => {
                let cleanup = self.release(resource);
                match (result, cleanup) {
                    (Err(error), Err(cleanup)) => Err(anyhow!("{error:#}；{cleanup:#}")),
                    (Err(error), Ok(())) => Err(error),
                    (_, Err(cleanup)) => Err(cleanup),
                    _ => Ok(None),
                }
            }
        }
    }
    pub fn tick(
        &self,
        hal: &crate::hal::Hal,
        permit: impl FnMut() -> Result<bool>,
    ) -> Result<Option<String>> {
        self.poll(&native::Native(hal), permit)
    }
    fn poll(
        &self,
        io: &impl Io,
        mut permit: impl FnMut() -> Result<bool>,
    ) -> Result<Option<String>> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| anyhow!("屏幕按键执行状态异常"))?;
        if self.state()?.frozen {
            return Ok(None);
        }
        let Some(mut resource) = self.state()?.resource.take() else {
            return Ok(None);
        };
        let event = resource.poll();
        if matches!(event, Ok(Event::Pending)) {
            self.state()?.resource = Some(resource);
            return Ok(None);
        }
        let owned = self
            .state()?
            .owned
            .clone()
            .context("屏幕按键会话身份缺失")?;
        let event = match (event, self.release(resource)) {
            (Err(error), Err(cleanup)) => bail!("{error:#}；{cleanup:#}"),
            (Err(error), _) | (_, Err(error)) => return Err(error),
            (Ok(event), Ok(())) => event,
        };
        match event {
            Event::Pending => unreachable!(),
            Event::Exited(0) if owned.kind == Kind::BlankSaver => {
                Ok(Some("本软件空白屏保已结束".into()))
            }
            Event::Exited(code) => bail!("屏幕按键子进程异常退出：{code}"),
            Event::Input(activity) if owned.kind == Kind::PointerWake => {
                if !requests_wake(activity) {
                    return Ok(Some("键盘活动已结束本次输入唤醒等待".into()));
                }
                if !permit()? {
                    return Ok(None);
                }
                let source = io.current()?;
                source.require(&owned.identity)?;
                if !permit()? {
                    return Ok(None);
                }
                io.wake(&source)?;
                Ok(Some("屏幕唤醒请求已发送，未验证物理显示状态".into()))
            }
            Event::Input(_) => bail!("屏幕按键会话返回了不适用的输入事件"),
        }
    }
}
