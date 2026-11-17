use super::model::*;
use crate::hal::screen_brightness::Reading;
use anyhow::{Result, bail};
use std::collections::BTreeMap;

pub(super) trait Io {
    fn snapshot(&self) -> Result<Snapshot>;
    fn panel(&self) -> Result<Reading>;
    fn active(&self) -> Result<u128>;
    fn levels(&self, plan: u128) -> Result<[u32; 2]>;
    fn write(&self, plan: u128, ac: bool, value: u32) -> Result<()>;
    fn activate(&self, plan: u128) -> Result<()>;
    fn ensure_writable(&self) -> Result<()>;
}
#[derive(Default)]
pub(super) struct State {
    pub initial: BTreeMap<u128, Baseline>,
}
impl State {
    pub fn compatible(&self, panel: &str) -> Result<()> {
        if self
            .initial
            .values()
            .any(|baseline| baseline.panel != panel)
        {
            bail!("存在另一面板的亮度恢复记录，请连接原面板后恢复");
        }
        Ok(())
    }
    pub fn preview(&self, io: &impl Io) -> Profile {
        let snapshot = io.snapshot().and_then(|snapshot| {
            snapshot.validate()?;
            self.compatible(&snapshot.reading.panel)?;
            Ok(snapshot)
        });
        let identity = snapshot.as_ref().ok().and_then(|s| s.identity().ok());
        let panel = snapshot
            .as_ref()
            .map(|s| s.reading.clone())
            .map_err(|e| e.to_string())
            .or_else(|_| io.panel().map_err(|e| format!("{e:#}")));
        let initial = self
            .initial
            .iter()
            .map(|(plan, baseline)| {
                let current = io.levels(*plan).and_then(|levels| {
                    if levels.iter().any(|v| *v > 100) {
                        bail!("恢复计划亮度报告无效");
                    }
                    Ok(levels)
                });
                Initial {
                    plan: format!("{plan:032x}"),
                    original: baseline.levels,
                    owned: baseline.owned,
                    current: current.as_ref().ok().copied(),
                    error: current.err().map(|e| format!("{e:#}")),
                }
            })
            .collect::<Vec<_>>();
        let restore_error = if self.initial.is_empty() {
            None
        } else {
            panel
                .as_ref()
                .map_err(|e| e.clone())
                .and_then(|p| self.compatible(&p.panel).map_err(|e| e.to_string()))
                .err()
                .or_else(|| initial.iter().find_map(|i| i.error.clone()))
        };
        let restore_revision = if !initial.is_empty() && restore_error.is_none() {
            super::super::model::digest(&(&initial, panel.as_ref().ok().map(|p| &p.panel))).ok()
        } else {
            None
        };
        Profile {
            identity,
            current: snapshot.as_ref().ok().map(Snapshot::view),
            error: snapshot.err().map(|e| format!("{e:#}")),
            initial,
            restore_revision,
            restore_error,
        }
    }
    pub fn step(
        &mut self,
        io: &impl Io,
        job: &mut Job,
        write_allowed: bool,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<bool> {
        let snapshot = io.snapshot()?;
        if snapshot.identity()? != job.identity {
            bail!("屏幕亮度执行来源变化，已停止后续输出");
        }
        self.compatible(&snapshot.reading.panel)?;
        if snapshot.reading.on_ac || snapshot.reading.current <= job.target {
            return Ok(true);
        }
        if !write_allowed {
            bail!("屏幕实际亮度未达到原厂目标，已停止重复写入");
        }
        io.ensure_writable()?;
        permit()?;
        for index in 0..2 {
            permit()?;
            let before = *job.before.get_or_insert(snapshot.levels);
            let initial = self.initial.entry(job.plan).or_insert_with(|| Baseline {
                panel: job.panel.clone(),
                levels: before,
                owned: [false; 2],
            });
            job.owned[index] = true;
            initial.owned[index] = true;
            io.write(job.plan, index == 0, u32::from(job.target))?;
            if io.levels(job.plan)?[index] != u32::from(job.target) {
                bail!("屏幕计划亮度回读不一致");
            }
        }
        permit()?;
        if io.active()? != job.plan {
            bail!("活动电源计划变化，未刷新旧计划");
        }
        io.activate(job.plan)?;
        if io.levels(job.plan)? != [u32::from(job.target); 2] {
            bail!("屏幕计划亮度组合回读不一致");
        }
        permit()?;
        Ok(false)
    }
    pub fn rollback(&self, io: &impl Io, job: &Job) -> Result<()> {
        if let Some(before) = job.before {
            restore_fields(io, &job.panel, job.plan, before, job.owned)?;
        }
        Ok(())
    }
    pub fn restore(&self, io: &impl Io, expected: &str) -> Result<()> {
        let preview = self.preview(io);
        if preview.restore_revision.as_deref() != Some(expected) {
            bail!("初始屏幕计划亮度预览已变化，请重新查看");
        }
        let mut errors = Vec::new();
        for (plan, baseline) in &self.initial {
            if let Err(error) =
                restore_fields(io, &baseline.panel, *plan, baseline.levels, baseline.owned)
            {
                errors.push(format!("计划{plan:032x}：{error:#}"));
            }
        }
        if !errors.is_empty() {
            bail!("屏幕计划亮度恢复未完成：{}", errors.join("；"));
        }
        Ok(())
    }
}
fn restore_fields(
    io: &impl Io,
    panel: &str,
    plan: u128,
    levels: [u32; 2],
    owned: [bool; 2],
) -> Result<()> {
    if !owned.into_iter().any(|field| field) {
        return Ok(());
    }
    io.ensure_writable()?;
    if io.panel()?.panel != panel {
        bail!("原亮度面板已变化，保留恢复记录");
    }
    let mut errors = Vec::new();
    for index in 0..2 {
        if !owned[index] {
            continue;
        }
        if let Err(error) = io.write(plan, index == 0, levels[index]).and_then(|_| {
            if io.levels(plan)?[index] != levels[index] {
                bail!("初始亮度字段回读不一致");
            }
            Ok(())
        }) {
            errors.push(format!(
                "{}：{error:#}",
                if index == 0 { "插电" } else { "电池" }
            ));
        }
    }
    match io.active() {
        Ok(active) if active == plan => {
            if let Err(error) = io.activate(plan) {
                errors.push(format!("刷新计划：{error:#}"));
            }
        }
        Ok(_) => (),
        Err(error) => errors.push(format!("恢复后无法核对活动计划：{error:#}")),
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("；"));
    }
    Ok(())
}
