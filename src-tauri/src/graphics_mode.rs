//! 原厂模式选择：命令接受与重启后的固件报告分开验证。
use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Mutex};
#[path = "graphics_mode/model.rs"]
mod model;
#[path = "graphics_mode/native.rs"]
mod native;
#[path = "graphics_mode/profile.rs"]
pub(crate) mod profile;
#[path = "graphics_mode/scope.rs"]
mod scope;
#[path = "graphics_mode/store.rs"]
mod store;
#[path = "graphics_mode/transaction.rs"]
mod transaction;
pub(crate) use model::Verification;
use model::{Identity, LastRequest, Observed, Phase, Record};
pub use model::{Profile, Request};
pub(crate) use scope::Scope;
use store::{FileStore, Loaded, Store};
#[cfg(test)]
#[path = "graphics_mode/profile_tests.rs"]
mod profile_tests;
#[cfg(test)]
#[path = "graphics_mode/test_fixture.rs"]
pub(crate) mod test_fixture;
#[cfg(test)]
#[path = "graphics_mode/tests.rs"]
mod tests;

pub(crate) trait Io {
    fn observed(&self) -> Result<Option<Observed>>;
    fn identity(&self) -> Result<Identity>;
    fn send(&self, target: u8) -> Result<()>;
    fn reboot(&self) -> Result<()>;
}
#[derive(Clone)]
struct Preview {
    observed: Observed,
    identity: Identity,
    loaded: Loaded,
    revision: String,
}
fn preview(io: &(impl Io + ?Sized), store: &impl Store) -> Result<Option<Preview>> {
    let Some(observed) = io.observed()? else {
        return Ok(None);
    };
    observed.validate()?;
    let identity = io.identity()?;
    identity.validate()?;
    let loaded = store.load()?;
    if let Some(record) = &loaded.record {
        if record.owner != identity.owner {
            bail!("显卡选择记录属于其他Windows安装，未覆盖记录");
        }
        if record.available_mask != observed.available_mask && record.boot == identity.boot {
            bail!("显卡能力已变化，旧选择记录未执行");
        }
    }
    let revision = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{}:{}:{}:{}",
                identity.owner,
                identity.boot,
                serde_json::to_string(&observed)?,
                loaded.revision
            )
            .as_bytes()
        )
    );
    Ok(Some(Preview {
        observed,
        identity,
        loaded,
        revision,
    }))
}
fn profile(io: &(impl Io + ?Sized), store: &impl Store) -> Result<Profile> {
    let Some(p) = preview(io, store)? else {
        return Ok(Profile {
            current: None,
            revision: "unsupported".into(),
            last_request: None,
            can_restore: false,
        });
    };
    Ok(describe(&p))
}
fn describe(p: &Preview) -> Profile {
    let last_request = p.loaded.record.as_ref().map(|r| LastRequest {
        initial_mode: r.initial_mode,
        target: r.target,
        verification: r.verification(&p.observed, &p.identity),
    });
    let can_restore = p.loaded.record.as_ref().is_some_and(|r| {
        p.observed.supports(r.initial_mode)
            && (r.target != r.initial_mode
                || r.phase == Phase::Prepared
                    && r.verification(&p.observed, &p.identity) != Verification::Confirmed)
    });
    Profile {
        current: Some(p.observed.clone()),
        revision: p.revision.clone(),
        last_request,
        can_restore,
    }
}
fn check_revision(p: &Preview, expected: &str) -> Result<()> {
    if p.revision != expected {
        bail!("显卡模式、启动状态或选择记录已变化，请重新读取并确认");
    }
    Ok(())
}
fn pending(p: &Preview, target: u8, restoring: bool) -> Record {
    let previous_mode = p
        .loaded
        .record
        .as_ref()
        .filter(|r| r.boot == p.identity.boot)
        .map(Record::previous_known)
        .unwrap_or(p.observed.mode);
    let initial_mode = p
        .loaded
        .record
        .as_ref()
        .filter(|r| restoring || r.boot == p.identity.boot)
        .map(|r| r.initial_mode)
        .unwrap_or(p.observed.mode);
    Record {
        version: 1,
        owner: p.identity.owner.clone(),
        boot: p.identity.boot.clone(),
        initial_mode,
        previous_mode,
        target,
        available_mask: p.observed.available_mask,
        phase: Phase::Prepared,
    }
}
fn send(io: &(impl Io + ?Sized), p: &Preview, target: u8) -> Result<()> {
    if io.identity()? != p.identity {
        bail!("Windows安装或系统启动已变化，未发送显卡选择");
    }
    let actual = io.observed()?.context("显卡模式接口已不可用")?;
    if actual.available_mask != p.observed.available_mask || !actual.supports(target) {
        bail!("显卡能力已变化，未发送选择");
    }
    io.send(target)?;
    let after = io.observed()?.context("发送后显卡模式接口不可用")?;
    if io.identity()? != p.identity {
        bail!("显卡选择后系统启动标识已变化");
    }
    if after.available_mask != p.observed.available_mask {
        bail!("发送后显卡能力已变化");
    }
    Ok(())
}
fn commit(
    io: &(impl Io + ?Sized),
    store: &impl Store,
    p: &Preview,
    target: u8,
    restoring: bool,
) -> Result<()> {
    let mut plan = transaction::Plan::new(p.clone(), target, restoring)?;
    if let Err(error) = plan.execute(io, store) {
        if !plan.dirty() {
            return Err(error).context("显卡选择未执行");
        }
        match plan.rollback(io, store) {
            Ok(()) => bail!("显卡选择未完成：{error:#}；已发送此前选择的恢复请求，重启后核对"),
            Err(restore) => bail!(
                "显卡选择未完成：{error:#}；恢复未完成：{restore:#}。记录已保留，请重新读取后恢复"
            ),
        }
    }
    Ok(())
}

fn apply(io: &(impl Io + ?Sized), store: &impl Store, request: &Request) -> Result<()> {
    let p = preview(io, store)?.context("本机未提供已确认的原厂显卡模式接口")?;
    check_revision(&p, &request.expected_revision)?;
    if p.observed.mode == request.target && p.loaded.record.is_none() {
        return Ok(());
    }
    if p.loaded.record.as_ref().is_some_and(|r| {
        r.target == request.target
            && r.verification(&p.observed, &p.identity) == Verification::Confirmed
    }) {
        return Ok(());
    }
    commit(io, store, &p, request.target, false)
}
fn restore(io: &(impl Io + ?Sized), store: &impl Store, expected: &str) -> Result<()> {
    let p = preview(io, store)?.context("本机未提供已确认的原厂显卡模式接口")?;
    check_revision(&p, expected)?;
    let target = p
        .loaded
        .record
        .as_ref()
        .context("没有可恢复的显卡选择记录")?
        .initial_mode;
    commit(io, store, &p, target, true)
}
fn reboot(io: &(impl Io + ?Sized), store: &impl Store, expected: &str) -> Result<()> {
    let p = preview(io, store)?.context("本机未提供已确认的原厂显卡模式接口")?;
    check_revision(&p, expected)?;
    let record = p.loaded.record.as_ref().context("没有待重启的显卡选择")?;
    if record.verification(&p.observed, &p.identity) != Verification::PendingReboot {
        bail!("请先确认并发送显卡选择，再请求重启");
    }
    io.reboot()
        .context("Windows重启请求失败；选择记录仍保留，可重试或恢复")
}
pub struct Controller {
    store: FileStore,
    gate: Mutex<()>,
    #[cfg(test)]
    io: Option<std::sync::Arc<dyn Io + Send + Sync>>,
}
impl Controller {
    pub fn new(path: PathBuf) -> Self {
        Self {
            store: FileStore(path),
            gate: Mutex::new(()),
            #[cfg(test)]
            io: None,
        }
    }
    pub fn read(&self, hal: &crate::hal::Hal) -> Result<Profile> {
        let scope = self.scope()?;
        let _hardware = hal.begin_write()?;
        scope.read(hal)
    }
    pub fn apply(
        &self,
        hal: &crate::hal::Hal,
        display: &crate::display::Controller,
        request: &Request,
    ) -> Result<()> {
        let scope = self.scope()?;
        let _hardware = hal.begin_write()?;
        allowed(hal, display)?;
        scope.with_io(hal, |io| apply(io, &self.store, request))
    }
    pub fn restore(
        &self,
        hal: &crate::hal::Hal,
        display: &crate::display::Controller,
        expected: &str,
    ) -> Result<()> {
        let scope = self.scope()?;
        let _hardware = hal.begin_write()?;
        allowed(hal, display)?;
        scope.with_io(hal, |io| restore(io, &self.store, expected))
    }
    pub fn reboot(
        &self,
        hal: &crate::hal::Hal,
        display: &crate::display::Controller,
        expected: &str,
    ) -> Result<()> {
        let scope = self.scope()?;
        let _hardware = hal.begin_write()?;
        allowed(hal, display)?;
        scope.with_io(hal, |io| reboot(io, &self.store, expected))
    }
}
fn allowed(hal: &crate::hal::Hal, display: &crate::display::Controller) -> Result<()> {
    if display.trial_pending()? {
        bail!("请先保留或恢复刷新率试用，再修改显卡模式");
    }
    if crate::fanctl::software::read(hal)?.is_some() {
        bail!("请先退出软件风扇曲线，再修改显卡模式");
    }
    Ok(())
}
