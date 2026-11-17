use super::*;

pub(super) fn emit_raw(io: &impl LightingIo, base: &Baseline, block: &SavedBlock) -> Result<()> {
    let request = block.request(base.family)?;
    let enabled = block.bytes[idx(80)] != 0;
    match request {
        LightingRequest::White { brightness } => {
            io.write_word(CMD_WHITE, u32::from(if enabled { brightness } else { 0 }))
        }
        LightingRequest::ZoneRgb {
            brightness,
            effect,
            all_color,
            zones,
        } => {
            io.write_word(
                CMD_RGB,
                encode_rgb_brightness_word(if enabled { brightness } else { 0 }, base.raw_type)?,
            )?;
            if matches!(effect, Effect::Zones | Effect::Breath | Effect::Solid) {
                for (n, zone) in zones.iter().enumerate() {
                    let color = if effect == Effect::Solid {
                        all_color
                    } else {
                        zone.color
                    };
                    io.write_word(
                        CMD_RGB,
                        encode_zone_color(0xF0 + n as u8, color, base.raw_type),
                    )?;
                }
            }
            if let Some(word) = effect.word() {
                io.write_word(CMD_RGB, word)?;
            }
            io.write_word(
                CMD_RGB,
                encode_onoff_mask(block, &zones, enabled, base.light_bar),
            )
        }
    }
}

pub(super) fn apply_request(
    io: &impl LightingIo,
    baseline: &mut Option<Baseline>,
    request: &LightingRequest,
    raw_type: Option<u8>,
) -> Result<(), ApplyError> {
    let raw_type = raw_type.ok_or_else(|| ApplyError {
        message: "键盘类型未知".into(),
        rollback: None,
    })?;
    let target = Target {
        keyboard_type: raw_type,
        enabled: request.enabled(),
        settings: request.clone(),
    };
    apply_target(io, baseline, &target, &mut false)
}
pub(super) fn require_baseline(
    baseline: &Option<Baseline>,
    current: &Baseline,
) -> Result<(), ApplyError> {
    if let Some(initial) = baseline.as_ref() {
        if initial.family != current.family
            || initial.raw_type != current.raw_type
            || initial.light_bar != current.light_bar
        {
            return Err(ApplyError {
                message: "键盘类型与本会话初始状态不一致".into(),
                rollback: None,
            });
        }
    }
    Ok(())
}
pub(super) fn apply_target(
    io: &impl LightingIo,
    baseline: &mut Option<Baseline>,
    target: &Target,
    needs_rollback: &mut bool,
) -> Result<(), ApplyError> {
    target.validate().map_err(|e| ApplyError {
        message: e.to_string(),
        rollback: None,
    })?;
    let current = capture(io, target.settings.family(), Some(target.keyboard_type))?;
    require_baseline(baseline, &current)?;
    let patches = target.patches();
    let mut after = SavedBlock::from_bytes(current.block.clone()).expect("已校验块长度");
    for patch in &patches {
        let start = idx(patch.offset);
        after.bytes[start..start + patch.data.len()].copy_from_slice(&patch.data);
    }
    if after.bytes == current.block {
        return Ok(());
    }
    baseline.get_or_insert_with(|| current.clone());
    *needs_rollback = true;
    if let Err(error) = emit_raw(io, &current, &after).and_then(|_| io.apply_saved(&patches)) {
        let restored = restore_snapshot(io, &current).map_err(|error| format!("{error:#}"));
        if restored.is_ok() {
            *needs_rollback = false;
        }
        return Err(ApplyError {
            message: format!(
                "键盘灯写入失败：{error:#}{}",
                if restored.is_ok() {
                    "；已恢复本次写入前配置"
                } else {
                    ""
                }
            ),
            rollback: restored.err(),
        });
    }
    Ok(())
}

pub(super) fn capture(
    io: &impl LightingIo,
    family: KeyboardFamily,
    raw_type: Option<u8>,
) -> Result<Baseline, ApplyError> {
    let capture = || -> Result<Baseline> {
        let block = io.read_saved_block()?;
        block.request(family)?;
        Ok(Baseline {
            family,
            raw_type,
            light_bar: if family == KeyboardFamily::ZoneRgb {
                io.light_bar(raw_type)?
            } else {
                false
            },
            block: block.bytes,
            cycle_index: None,
        })
    };
    capture().map_err(|error| ApplyError {
        message: format!("写入前快照不可恢复（未写入）：{error:#}"),
        rollback: None,
    })
}

pub(super) fn restore_initial(io: &impl LightingIo, baseline: &Option<Baseline>) -> ApplyOutcome {
    let Some(captured) = baseline else {
        return failure("没有可恢复的基准：本会话尚未写入过键盘灯".into());
    };
    match restore_snapshot(io, captured) {
        Ok(()) => ApplyOutcome {
            ok: true,
            message: "已恢复到本应用首次写入前的键盘灯配置".into(),
            rollback_error: None,
        },
        Err(error) => failure(format!("{error:#}")),
    }
}

pub(super) fn restore_snapshot(io: &impl LightingIo, captured: &Baseline) -> Result<()> {
    if captured.family == KeyboardFamily::ZoneRgb {
        match io.light_bar(captured.raw_type) {
            Ok(value) if value == captured.light_bar => (),
            Ok(_) => anyhow::bail!("外围灯区能力已变化，拒绝旧灯光恢复"),
            Err(error) => anyhow::bail!("灯光恢复能力读取失败：{error:#}"),
        }
    }
    let patches = owned_patches(captured);
    let raw = (|| {
        let mut current = io.read_saved_block()?;
        for patch in patches.iter().filter(|patch| patch.offset != 127) {
            let start = idx(patch.offset);
            current.bytes[start..start + patch.data.len()].copy_from_slice(&patch.data);
        }
        emit_raw(io, captured, &current)
    })();
    let saved = io.apply_saved(&patches);
    let errors: Vec<_> = [
        raw.err().map(|e| format!("立即通道回放基线：{e:#}")),
        saved.err().map(|e| format!("保存值回写基线：{e:#}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !errors.is_empty() {
        anyhow::bail!(errors.join("；"));
    }
    Ok(())
}

fn owned_patches(base: &Baseline) -> Vec<Patch> {
    let ranges: &[(usize, usize)] = match base.family {
        KeyboardFamily::White => &[(35, 1), (80, 1)],
        KeyboardFamily::ZoneRgb => &[(32, 1), (35, 1), (80, 16)],
        _ => &[],
    };
    let mut patches: Vec<_> = ranges
        .iter()
        .map(|&(offset, length)| {
            Patch::new(
                offset,
                base.block[idx(offset)..idx(offset) + length].to_vec(),
            )
        })
        .collect();
    if let Some(index) = base.cycle_index {
        patches.push(Patch::new(127, [index]));
    }
    patches
}
