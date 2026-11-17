use super::{ClockRead, FileStore, Io, read, record};
use anyhow::{Result, anyhow, bail};

pub(in crate::powerctl::mode::supply) fn restore(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let view = read(io, store)?;
    if view.revision != expected || !view.can_restore {
        bail!("供电模式恢复预览已变化或不可恢复");
    }
    let mut record = store
        .load()?
        .ok_or_else(|| anyhow!("供电模式恢复记录缺失"))?;
    if record.enabled {
        record::update(store, &mut record, |record| record.enabled = false)?;
    }
    let Some(initial) = record.initial.clone() else {
        return store.clear(&record);
    };
    let capture = record
        .capture_target
        .clone()
        .ok_or_else(|| anyhow!("恢复快照缺少采样目标"))?;
    let actual = view
        .current
        .ok_or_else(|| anyhow!("供电模式恢复前态不可用"))?;
    if io.device_identity()? != record.device_identity {
        bail!("设备已变化，未恢复原模式");
    }
    permit()?;
    io.restore(&initial, &capture, &actual, permit)?;
    if super::capture(
        io,
        &capture,
        &initial.plans,
        ClockRead::for_observation(&initial.expected().gpu_clocks)?,
    )? != initial.expected()
    {
        bail!("模式恢复回读不一致，保留恢复记录");
    }
    store.clear(&record)
}
