use super::{model::*, store::Loaded};
use anyhow::{Result, bail};

pub(super) trait Io {
    type Prepared;
    fn load(&self) -> Result<Loaded>;
    fn facts(&self) -> Facts;
    fn stop(&self) -> Result<bool>;
    fn prepare(&self, record: &Record) -> Result<Self::Prepared>;
    fn activate(&self, prepared: Self::Prepared) -> Result<()>;
    fn save(&self, before: &Loaded, target: &Record) -> Result<Loaded>;
}
pub(super) fn revision(loaded: &Loaded, facts: &Facts) -> Result<String> {
    digest(&(&loaded.revision, facts))
}
pub(super) fn restore_revision(loaded: &Loaded, initial: &Record) -> Result<String> {
    digest(&(&loaded.revision, initial))
}
pub(super) fn change(
    io: &impl Io,
    expected: &str,
    target: impl FnOnce(&Record, &Facts) -> Result<Record>,
    baseline: &mut Option<Record>,
) -> Result<()> {
    let loaded = io.load()?;
    let facts = io.facts();
    if revision(&loaded, &facts)? != expected {
        bail!("热键策略或设备预览已变化，请重新读取确认");
    }
    let next = target(&loaded.record, &facts)?;
    commit(io, loaded, facts, next, baseline)
}

pub(super) fn restore(io: &impl Io, expected: &str, baseline: &mut Option<Record>) -> Result<()> {
    let loaded = io.load()?;
    let Some(initial) = baseline.as_ref() else {
        bail!("尚未修改热键软件策略");
    };
    if restore_revision(&loaded, initial)? != expected {
        bail!("热键策略恢复目标已变化，请重新读取确认");
    }
    commit(io, loaded, io.facts(), initial.clone(), baseline)
}

fn commit(
    io: &impl Io,
    loaded: Loaded,
    facts: Facts,
    next: Record,
    baseline: &mut Option<Record>,
) -> Result<()> {
    next.validate()?;
    if next.settings.enabled {
        next.verify(&facts)?;
    }
    baseline.get_or_insert_with(|| loaded.record.clone());
    let was_running = io.stop()?;
    let mut committed = None;
    let result: Result<()> = (|| {
        let prepared = if next.settings.enabled {
            Some(io.prepare(&next)?)
        } else {
            None
        };
        committed = Some(io.save(&loaded, &next)?);
        if let Some(prepared) = prepared {
            io.activate(prepared)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut failures = Vec::new();
        if let Err(error) = io.stop() {
            failures.push(format!("停止新监听：{error:#}"));
        }
        if let Some(current) = committed {
            if let Err(error) = io.save(&current, &loaded.record) {
                failures.push(format!("恢复原策略：{error:#}"));
            }
        }
        if was_running {
            let restart = (|| {
                if io.load()?.record != loaded.record {
                    bail!("原策略文件已被修改");
                }
                loaded.record.verify(&io.facts())?;
                io.activate(io.prepare(&loaded.record)?)
            })();
            if let Err(error) = restart {
                failures.push(format!("恢复原监听：{error:#}"));
            }
        }
        bail!(
            "{error:#}{}",
            if failures.is_empty() {
                String::new()
            } else {
                format!("；{}", failures.join("；"))
            }
        );
    }
    Ok(())
}
