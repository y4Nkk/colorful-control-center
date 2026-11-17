use super::*;

#[derive(Clone, Copy)]
pub(super) enum Part {
    Timer,
    Boot,
    Logo,
    LightBar,
    TopCover,
}
pub(super) fn changes(before: &Snapshot, after: &Snapshot) -> Result<Vec<Part>> {
    let (a, b) = (before.values()?, after.values()?);
    Ok([
        (a.idle_timer != b.idle_timer, Part::Timer),
        (a.boot_override != b.boot_override, Part::Boot),
        (a.logo != b.logo, Part::Logo),
        (a.light_bar != b.light_bar, Part::LightBar),
        (a.top_cover != b.top_cover, Part::TopCover),
    ]
    .into_iter()
    .filter_map(|(changed, part)| changed.then_some(part))
    .collect())
}
fn patches(target: &Snapshot, parts: &[Part]) -> Vec<Patch> {
    parts
        .iter()
        .map(|part| {
            let (offset, length) = match part {
                Part::Timer => (36, 4),
                Part::Boot => (7, 1),
                Part::Logo => (100, 4),
                Part::LightBar => (104, 4),
                Part::TopCover => (100, 4),
            };
            Patch::new(offset, target.page[offset..offset + length].to_vec())
        })
        .collect()
}
pub(super) fn commands(target: &Snapshot, parts: &[Part]) -> Result<Vec<(i32, u32)>> {
    let values = target.values()?;
    let mut words = Vec::new();
    let mut mask = false;
    for part in parts {
        match part {
            Part::Timer => words.push((
                121,
                0x1800_0000
                    | if values.idle_timer.enabled {
                        u32::from(values.idle_timer.seconds) << 8 | 255
                    } else {
                        0
                    },
            )),
            Part::Boot => words.push((
                121,
                0x1E00_0000 | u32::from(values.boot_override.context("开机覆盖不支持")?),
            )),
            Part::TopCover => (),
            Part::Logo | Part::LightBar => {
                let (selector, zone) = match part {
                    Part::Logo => (0xF6, values.logo),
                    _ => (0xF3, values.light_bar),
                };
                words.push((
                    103,
                    encode_zone_color(
                        selector,
                        zone.context("外围灯区不支持")?.color,
                        Some(target.caps.keyboard_type),
                    ),
                ));
                mask = true;
            }
        }
    }
    if mask {
        for at in [80, 84, 88, 92, 96, 104] {
            if target.page[at] > 1 {
                bail!("全局灯光开关未知，未写入");
            }
        }
        if !target.caps.top_cover && target.page[100] > 1 {
            bail!("Logo保存开关未知，未写入");
        }
        let block = SavedBlock::from_bytes(target.page[32..108].to_vec())?;
        words.push((
            103,
            encode_onoff_mask(
                &block,
                &block.snapshot().zones,
                target.page[80] != 0,
                target.caps.light_bar,
            ),
        ));
    }
    if parts.iter().any(|part| matches!(part, Part::TopCover)) {
        words.extend(cover::commands(
            values.top_cover.as_ref().context("独立顶盖不支持")?,
        )?);
    }
    Ok(words)
}
fn compensate(io: &impl Io, target: &Snapshot, parts: &[Part]) -> Result<()> {
    let mut errors = Vec::new();
    let commands = match commands(target, parts) {
        Ok(commands) => commands,
        Err(error) => {
            errors.push(format!("灯光恢复编码：{error:#}"));
            vec![]
        }
    };
    for (command, word) in commands {
        if let Err(error) = io.send(command, word) {
            errors.push(format!("灯光命令恢复：{error:#}"));
        }
    }
    if let Err(error) = settings::apply(io, 2, &patches(target, parts), None) {
        errors.push(format!("灯光保存恢复：{error}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
pub(super) fn apply(
    io: &impl Io,
    baseline: &mut Option<Snapshot>,
    request: &Request,
) -> Result<()> {
    apply_tracked(io, baseline, request, &mut false)
}
pub(super) fn apply_tracked(
    io: &impl Io,
    baseline: &mut Option<Snapshot>,
    request: &Request,
    needs_rollback: &mut bool,
) -> Result<()> {
    let before = capture(io)?;
    if before.revision() != request.expected_revision {
        bail!("灯光选项或共享设置已变化，请重新预览");
    }
    if let Some(initial) = baseline.as_ref() {
        before.require_same_device(initial)?;
    }
    let target = before.target(&request.values)?;
    let parts = changes(&before, &target)?;
    if parts.is_empty() {
        return Ok(());
    }
    let words = commands(&target, &parts)?;
    commands(&before, &parts)?;
    baseline.get_or_insert_with(|| before.clone());
    *needs_rollback = true;
    let write = || -> Result<()> {
        for (command, word) in words {
            io.send(command, word)?;
        }
        settings::apply(io, 2, &patches(&target, &parts), None).map_err(|e| anyhow!(e))?;
        Ok(())
    };
    if let Err(error) = write() {
        let recovery = compensate(io, &before, &parts).map_err(|e| format!("{e:#}"));
        if recovery.is_ok() {
            *needs_rollback = false;
        }
        bail!(ccc_core::msr::with_recovery(
            format!("灯光选项写入失败：{error:#}"),
            recovery
        ));
    }
    Ok(())
}
pub(super) fn restore_parts(io: &impl Io, original: &Snapshot, parts: &[Part]) -> Result<()> {
    let mut current = capture_raw(io)?;
    current.require_same_device(original)?;
    for patch in patches(original, parts) {
        current.page[patch.offset..patch.offset + patch.data.len()].copy_from_slice(&patch.data);
    }
    compensate(io, &current, parts)
}
pub(super) fn restore(
    io: &impl Io,
    baseline: &Option<Snapshot>,
    expected_revision: &str,
) -> Result<()> {
    let before = capture(io)?;
    if before.revision() != expected_revision {
        bail!("灯光恢复预览已变化，请重新读取");
    }
    let initial = baseline.as_ref().context("尚无灯光选项初始快照")?;
    before.require_same_device(initial)?;
    let target = before.target(&initial.values()?)?;
    let mut parts = vec![Part::Timer];
    if target.caps.boot_override {
        parts.push(Part::Boot);
    }
    if target.caps.logo {
        parts.push(Part::Logo);
    }
    if target.caps.light_bar {
        parts.push(Part::LightBar);
    }
    if target.caps.top_cover {
        parts.push(Part::TopCover);
    }
    commands(&target, &parts)?;
    compensate(io, &target, &parts)
}
