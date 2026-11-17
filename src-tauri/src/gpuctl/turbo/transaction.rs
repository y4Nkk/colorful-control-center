use super::{model::*, source::SourceIo};
use crate::gpuctl::GpuCheckpoint;
use crate::hal::nvapi::{
    clock::LockEntry,
    transaction::{self, RawOffsets},
    turbo::{Offsets, TurboIo},
};
use anyhow::{Context, Result, anyhow, bail};

#[derive(Clone, Default)]
pub struct State {
    pub initial: Option<Initial>,
}

#[derive(Default)]
struct Plan {
    locks: Option<[LockEntry; 2]>,
    offsets: Vec<RawOffsets>,
}

fn plan(snapshot: &Snapshot, event: u8) -> Result<Plan> {
    let target = snapshot.target(event, snapshot.source.mode, snapshot.source.on_ac)?;
    let locks = target
        .lock_mhz
        .map(|mhz| snapshot.locks.original_target(mhz))
        .transpose()?
        .map(|table| table.owned())
        .transpose()?
        .filter(|wanted| snapshot.locks.owned().ok().as_ref() != Some(wanted));
    let mut offsets = vec![RawOffsets::default(); snapshot.offsets.len()];
    if let Some(targets) = target.offsets {
        if targets.len() != snapshot.offsets.len() {
            bail!("GPU 偏移目标数量变化");
        }
        for (index, (wanted, current)) in targets.iter().zip(&snapshot.offsets).enumerate() {
            let core =
                (raw(current).core_khz != Some(wanted.core_mhz * 1000)).then_some(wanted.core_mhz);
            let memory = (raw(current).mem_khz != Some(wanted.memory_mhz * 1000))
                .then_some(wanted.memory_mhz);
            if core.is_none() && memory.is_none() {
                continue;
            }
            let values = transaction::validate_requested(
                current,
                core,
                memory,
                super::super::core_caps(),
                super::super::mem_caps(),
            )
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("GPU {} 偏移目标无效", index + 1))?;
            offsets[index] = RawOffsets {
                core_khz: values
                    .iter()
                    .find(|(domain, _)| *domain == 0)
                    .map(|(_, value)| *value),
                mem_khz: values
                    .iter()
                    .find(|(domain, _)| *domain == 1)
                    .map(|(_, value)| *value),
            };
        }
    }
    Ok(Plan { locks, offsets })
}

pub(super) fn validate_plan(snapshot: &Snapshot, event: u8) -> Result<()> {
    plan(snapshot, event).map(|_| ())
}

pub(super) fn planned_state(snapshot: &Snapshot, event: u8) -> Result<super::mode::RawState> {
    plan(snapshot, event)?;
    let target = snapshot.target(event, snapshot.source.mode, snapshot.source.on_ac)?;
    super::mode::RawState::from_snapshot(snapshot)?.projected(&target)
}

fn check_current(io: &dyn TurboIo, source: &dyn SourceIo, expected: &Snapshot) -> Result<Snapshot> {
    let current = super::model::capture_with_base(io, source.read()?, expected.base_mhz.is_some())?;
    if current.identity()? != expected.identity()?
        || current.source.mode != expected.source.mode
        || current.source.on_ac != expected.source.on_ac
    {
        bail!("GPU Turbo 设备、能力、模式、供电或目标变化，已停止执行");
    }
    if current.locks.owned()? != expected.locks.owned()?
        || current.offsets.iter().map(raw).collect::<Vec<_>>()
            != expected.offsets.iter().map(raw).collect::<Vec<_>>()
    {
        bail!("GPU 锁频或偏移在执行中变化，已停止执行");
    }
    Ok(current)
}

impl State {
    fn compatible(&self, snapshot: &Snapshot) -> Result<()> {
        if self
            .initial
            .as_ref()
            .is_some_and(|initial| initial.devices != snapshot.devices)
        {
            bail!("存在其他 GPU 的初始恢复记录，请连接原设备后恢复");
        }
        Ok(())
    }

    fn initial_for(&mut self, snapshot: &Snapshot) -> &mut Initial {
        self.initial.get_or_insert_with(|| Initial {
            devices: snapshot.devices.clone(),
            locks: None,
            offsets: vec![RawOffsets::default(); snapshot.offsets.len()],
        })
    }

    fn remember_lock(&mut self, snapshot: &Snapshot) -> Result<()> {
        let current = snapshot.locks.owned()?;
        self.initial_for(snapshot).locks.get_or_insert(current);
        Ok(())
    }

    fn remember_offset(
        &mut self,
        snapshot: &Snapshot,
        shared: &mut Option<GpuCheckpoint>,
        index: usize,
        core: bool,
    ) -> Result<()> {
        let before = raw(&snapshot.offsets[index]);
        let value = if core {
            before.core_khz
        } else {
            before.mem_khz
        }
        .context("GPU 偏移前态不可读")?;
        let original = if index == 0 {
            let baseline = shared.get_or_insert_with(|| {
                GpuCheckpoint::new(snapshot.devices[0], RawOffsets::default())
            });
            if baseline.device != snapshot.devices[0] {
                bail!("GPU 初始偏移属于其他设备");
            }
            *if core {
                &mut baseline.core_khz
            } else {
                &mut baseline.mem_khz
            }
            .get_or_insert(value)
        } else {
            value
        };
        let baseline = &mut self.initial_for(snapshot).offsets[index];
        let field = if core {
            &mut baseline.core_khz
        } else {
            &mut baseline.mem_khz
        };
        field.get_or_insert(original);
        Ok(())
    }

    pub fn preview(&self, io: &dyn TurboIo, source: &dyn SourceIo) -> Profile {
        let snapshot = source
            .read()
            .and_then(|source| capture(io, source))
            .and_then(|snapshot| {
                self.compatible(&snapshot)?;
                Ok(snapshot)
            });
        let current = snapshot
            .as_ref()
            .map_err(|e| format!("{e:#}"))
            .and_then(|s| s.view().map_err(|e| format!("{e:#}")));
        let identity = current
            .as_ref()
            .ok()
            .and_then(|_| snapshot.as_ref().ok()?.identity().ok());
        let restore = self
            .initial
            .as_ref()
            .map(|initial| observe_restore(io, initial));
        let restore_current = restore.as_ref().and_then(|r| r.as_ref().ok()).cloned();
        let restore_revision = self
            .initial
            .as_ref()
            .zip(restore_current.as_ref())
            .and_then(|(initial, current)| digest(&("gpu-turbo-restore-1", initial, current)).ok());
        Profile {
            identity,
            current: current.as_ref().ok().cloned(),
            error: current.err(),
            initial: self.initial.clone(),
            restore_current,
            restore_revision,
            restore_error: restore.and_then(Result::err).map(|e| format!("{e:#}")),
        }
    }

    pub fn execute(
        &mut self,
        io: &dyn TurboIo,
        source: &dyn SourceIo,
        shared: &mut Option<GpuCheckpoint>,
        expected: &str,
        event: u8,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<String> {
        permit()?;
        let before = capture(io, source.read()?)?;
        if before.identity()? != expected {
            bail!("GPU Turbo 授权来源变化，请重新预览确认");
        }
        self.execute_captured(io, source, shared, &before, event, &mut permit)
    }

    pub(super) fn execute_captured(
        &mut self,
        io: &dyn TurboIo,
        source: &dyn SourceIo,
        shared: &mut Option<GpuCheckpoint>,
        before: &Snapshot,
        event: u8,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<String> {
        self.compatible(before)?;
        if shared.is_some_and(|baseline| baseline.device != before.devices[0]) {
            bail!("GPU 初始偏移属于其他设备，不能接管当前显卡");
        }
        let plan = plan(&before, event)?;
        let previous = self.clone();
        let previous_shared = *shared;
        let mut attempted_locks = false;
        let mut attempted_offsets = vec![RawOffsets::default(); before.offsets.len()];
        let result = self.run(
            io,
            source,
            shared,
            &before,
            &plan,
            &mut permit,
            &mut attempted_locks,
            &mut attempted_offsets,
        );
        match result {
            Ok(changed) => Ok(if changed {
                "已按原厂模式更新并回读 GPU 锁频与偏移"
            } else {
                "当前 GPU 状态无需修改"
            }
            .into()),
            Err(error) => match rollback(io, &before, attempted_locks, &attempted_offsets) {
                Ok(()) => {
                    *self = previous;
                    *shared = previous_shared;
                    Err(error.context("GPU Turbo 未完成，本次已尝试字段已恢复"))
                }
                Err(restore) => Err(anyhow!(
                    "GPU Turbo 失败：{error:#}；本次恢复失败：{restore:#}"
                )),
            },
        }
    }

    fn run(
        &mut self,
        io: &dyn TurboIo,
        source: &dyn SourceIo,
        shared: &mut Option<GpuCheckpoint>,
        before: &Snapshot,
        plan: &Plan,
        permit: &mut impl FnMut() -> Result<()>,
        attempted_locks: &mut bool,
        attempted_offsets: &mut [RawOffsets],
    ) -> Result<bool> {
        let mut working = before.clone();
        let mut changed = false;
        if let Some(wanted) = plan.locks {
            permit()?;
            working = check_current(io, source, &working)?;
            let request =
                prepare_locks(io, &before.devices[0], wanted, Some(working.locks.owned()?))?;
            permit()?;
            self.remember_lock(before)?;
            *attempted_locks = true;
            send_locks(io, &before.devices[0], &request, wanted)?;
            working.locks = io.locks(&before.devices[0])?;
            changed = true;
        }
        for (index, target) in plan.offsets.iter().enumerate() {
            for (core, value) in [(true, target.core_khz), (false, target.mem_khz)] {
                let Some(value) = value else {
                    continue;
                };
                permit()?;
                working = check_current(io, source, &working)?;
                self.remember_offset(before, shared, index, core)?;
                let prior = raw(&before.offsets[index]);
                if core {
                    attempted_offsets[index].core_khz = prior.core_khz;
                } else {
                    attempted_offsets[index].mem_khz = prior.mem_khz;
                }
                write_offset(io, &before.devices[index], core, value)?;
                working.offsets[index] = io.offsets(&before.devices[index])?;
                changed = true;
            }
        }
        permit()?;
        check_current(io, source, &working)?;
        Ok(changed)
    }

    pub fn restore(&mut self, io: &dyn TurboIo, expected: &str) -> Result<()> {
        let initial = self
            .initial
            .as_ref()
            .context("没有可恢复的 GPU Turbo 初始记录")?;
        let current = observe_restore(io, initial)?;
        if digest(&("gpu-turbo-restore-1", initial, &current))? != expected {
            bail!("GPU Turbo 初始恢复预览已变化，请重新查看");
        }
        restore_fields(io, &initial.devices, initial.locks, &initial.offsets)?;
        self.initial = None;
        Ok(())
    }
}

pub(super) fn prepare_locks(
    io: &dyn TurboIo,
    id: &crate::hal::nvapi::turbo::DeviceId,
    wanted: [LockEntry; 2],
    expected: Option<[LockEntry; 2]>,
) -> Result<crate::hal::nvapi::clock::LockTable> {
    let current = io.locks(id)?.owned()?;
    if expected.is_some_and(|expected| expected != current) {
        bail!("GPU 锁频在准备写入时变化");
    }
    crate::hal::nvapi::clock::LockTable::request_owned(wanted)
}

pub(super) fn send_locks(
    io: &dyn TurboIo,
    id: &crate::hal::nvapi::turbo::DeviceId,
    table: &crate::hal::nvapi::clock::LockTable,
    wanted: [LockEntry; 2],
) -> Result<()> {
    io.write_locks(id, table).context("发送 GPU 锁频")?;
    if io.locks(id)?.owned()? != wanted {
        bail!("GPU 锁频回读与目标不一致");
    }
    Ok(())
}

pub(super) fn write_offset(
    io: &dyn TurboIo,
    id: &crate::hal::nvapi::turbo::DeviceId,
    core: bool,
    value: i32,
) -> Result<()> {
    let access = Offsets { io, id };
    transaction::write_and_verify(&access, &[(if core { 0 } else { 1 }, value)])?;
    Ok(())
}

fn rollback(
    io: &dyn TurboIo,
    before: &Snapshot,
    locks: bool,
    offsets: &[RawOffsets],
) -> Result<()> {
    let owned_locks = if locks {
        Some(before.locks.owned()?)
    } else {
        None
    };
    restore_fields(io, &before.devices, owned_locks, offsets)
}

fn restore_fields(
    io: &dyn TurboIo,
    devices: &[crate::hal::nvapi::turbo::DeviceId],
    locks: Option<[LockEntry; 2]>,
    offsets: &[RawOffsets],
) -> Result<()> {
    if locks.is_none()
        && offsets
            .iter()
            .all(|value| value.core_khz.is_none() && value.mem_khz.is_none())
    {
        return Ok(());
    }
    if io.devices()? != devices {
        bail!("GPU 设备变化，未向新设备发送恢复命令");
    }
    let mut errors = Vec::new();
    for index in (0..offsets.len()).rev() {
        for (core, value) in [
            (false, offsets[index].mem_khz),
            (true, offsets[index].core_khz),
        ] {
            if let Some(value) = value {
                if let Err(error) = same_devices(io, devices) {
                    errors.push(format!("设备复核：{error:#}；已停止剩余恢复命令"));
                    bail!("{}", errors.join("；"));
                }
                if let Err(error) = write_offset(io, &devices[index], core, value) {
                    errors.push(format!(
                        "GPU {} {}：{error:#}",
                        index + 1,
                        if core { "核心" } else { "显存" }
                    ));
                }
            }
        }
    }
    if let Some(locks) = locks {
        if let Err(error) = same_devices(io, devices) {
            errors.push(format!("设备复核：{error:#}；已停止剩余恢复命令"));
            bail!("{}", errors.join("；"));
        }
        let result = prepare_locks(io, &devices[0], locks, None)
            .and_then(|request| send_locks(io, &devices[0], &request, locks));
        if let Err(error) = result {
            errors.push(format!("锁频：{error:#}"));
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("；"));
    }
    Ok(())
}

fn same_devices(io: &dyn TurboIo, expected: &[crate::hal::nvapi::turbo::DeviceId]) -> Result<()> {
    if io.devices()? != expected {
        bail!("GPU 设备在恢复中变化");
    }
    Ok(())
}
