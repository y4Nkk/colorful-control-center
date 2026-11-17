use super::*;
use anyhow::bail;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize)]
struct Info {
    keyboard_type: u8,
    display_maximum: u8,
    light_bar: bool,
}
impl Info {
    fn identity(&self) -> Result<String> {
        if !matches!(
            (self.keyboard_type, self.display_maximum),
            (22, 4) | (1, 2 | 5)
        ) || self.keyboard_type == 1 && self.light_bar
        {
            bail!("此键盘未提供原厂亮度循环或反馈");
        }
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
}

pub(crate) struct Report {
    pub level: u8,
    pub display_maximum: u8,
    pub firmware_report: bool,
}

impl Report {
    pub(crate) fn show_osd(&self) -> bool {
        self.level <= self.display_maximum
    }
}

fn native_info(hal: &Hal) -> Result<Info> {
    let profile = hal.with_dchu(|d| Ok(build_profile(d, None)))?;
    hotkey::identity(&profile)?;
    match profile.detected_type {
        Some(22) => Ok(Info {
            keyboard_type: 22,
            display_maximum: 4,
            light_bar: DchuLighting(hal).light_bar(Some(22))?,
        }),
        Some(1) => hal.with_dchu(|d| {
            let flags = d.read_bios_capabilities(0, 10)?;
            Ok(Info {
                keyboard_type: 1,
                display_maximum: white_display_maximum(&flags)?,
                light_bar: false,
            })
        }),
        _ => bail!("此键盘未提供原厂亮度循环或反馈"),
    }
}

fn white_display_maximum(flags: &[u8]) -> Result<u8> {
    if flags.len() != 10 || flags[..2] != [1, 0] {
        bail!("白光档位能力页版本未知，未开放亮度反馈");
    }
    let bits = u32::from_le_bytes(flags[6..10].try_into()?);
    Ok(if bits & (1 << 14) != 0 { 5 } else { 2 })
}

fn patches(block: &SavedBlock) -> Vec<Patch> {
    [35, 80]
        .map(|at| Patch::new(at, [block.bytes[idx(at)]]))
        .into()
}

fn emit(io: &impl LightingIo, block: &SavedBlock, light_bar: bool, brightness: bool) -> Result<()> {
    let current = block.snapshot();
    if brightness {
        io.write_word(
            CMD_RGB,
            encode_rgb_brightness_word(current.brightness_level, Some(22))?,
        )?;
    }
    io.write_word(
        CMD_RGB,
        encode_onoff_mask(block, &current.zones, current.enabled, light_bar),
    )
}

fn restore(io: &impl LightingIo, before: &SavedBlock, light_bar: bool) -> Result<()> {
    if io.light_bar(Some(22))? != light_bar {
        bail!("外围灯区能力已变化，拒绝旧亮度恢复");
    }
    let raw = (|| {
        let mut current = io.read_saved_block()?;
        for at in [35, 80] {
            current.bytes[idx(at)] = before.bytes[idx(at)];
        }
        emit(io, &current, light_bar, true)
    })();
    let saved = io.apply_saved(&patches(before));
    let errors: Vec<_> = [raw.err(), saved.err()]
        .into_iter()
        .flatten()
        .map(|e| format!("{e:#}"))
        .collect();
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

fn cycle(
    io: &impl LightingIo,
    initial: &mut Option<Baseline>,
    info: &Info,
) -> Result<Option<Report>> {
    let before = transaction::capture(io, KeyboardFamily::ZoneRgb, Some(22))
        .map_err(|e| anyhow!(e.message))?;
    transaction::require_baseline(initial, &before).map_err(|e| anyhow!(e.message))?;
    if before.light_bar != info.light_bar {
        bail!("灯条共享开关能力变化，未执行亮度事件");
    }
    let previous = SavedBlock::from_bytes(before.block.clone())?;
    let current = previous.snapshot();
    let level = if !current.enabled {
        1
    } else {
        match current.brightness_level {
            1..=3 => current.brightness_level + 1,
            4 => 0,
            _ => return Ok(None),
        }
    };
    let mut after = SavedBlock::from_bytes(before.block.clone())?;
    if level != 0 {
        after.bytes[idx(35)] = level;
    }
    after.bytes[idx(80)] = u8::from(level != 0);
    initial.get_or_insert(before);
    let result = emit(io, &after, info.light_bar, level != 0)
        .and_then(|_| io.apply_saved(&patches(&after)))
        .and_then(|_| {
            if io.read_saved_block()?.bytes != after.bytes {
                bail!("亮度事件保存回读不一致");
            }
            Ok(())
        });
    if let Err(error) = result {
        bail!(ccc_core::msr::with_recovery(
            format!("亮度循环失败：{error:#}"),
            restore(io, &previous, info.light_bar).map_err(|e| format!("{e:#}"))
        ));
    }
    Ok(Some(Report {
        level,
        display_maximum: 4,
        firmware_report: false,
    }))
}

fn synchronize(io: &impl LightingIo, info: &Info) -> Result<Option<Report>> {
    let raw = io.white_brightness()?;
    if !(0..=5).contains(&raw) {
        bail!("白光亮度报告超出协议范围：{raw}");
    }
    let before = io.read_saved_block()?;
    let mut after = SavedBlock::from_bytes(before.bytes.clone())?;
    after.bytes[idx(35)] = raw as u8;
    after.bytes[idx(80)] = u8::from(raw != 0);
    if after.bytes != before.bytes {
        let result = io.apply_saved(&patches(&after)).and_then(|_| {
            if io.read_saved_block()?.bytes != after.bytes {
                bail!("白光报告同步回读不一致");
            }
            Ok(())
        });
        if let Err(error) = result {
            bail!(ccc_core::msr::with_recovery(
                format!("白光报告同步失败：{error:#}"),
                io.apply_saved(&patches(&before))
                    .map_err(|e| format!("{e:#}"))
            ));
        }
    }
    Ok(Some(Report {
        level: raw as u8,
        display_maximum: info.display_maximum,
        firmware_report: true,
    }))
}

fn handle(
    io: &impl LightingIo,
    initial: &mut Option<Baseline>,
    info: &Info,
) -> Result<Option<Report>> {
    info.identity()?;
    match info.keyboard_type {
        22 => cycle(io, initial, info),
        1 => synchronize(io, info),
        _ => unreachable!(),
    }
}

impl LightingController {
    pub(crate) fn brightness_event_identity(&self, hal: &Hal) -> Result<String> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let info = native_info(hal)?;
        if info.keyboard_type == 22 {
            let before =
                transaction::capture(&DchuLighting(hal), KeyboardFamily::ZoneRgb, Some(22))
                    .map_err(|e| anyhow!(e.message))?;
            transaction::require_baseline(&scope.baseline, &before)
                .map_err(|e| anyhow!(e.message))?;
        }
        info.identity()
    }
    pub(crate) fn brightness_event(&self, hal: &Hal, expected: &str) -> Result<Option<Report>> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let info = native_info(hal)?;
        if info.identity()? != expected {
            bail!("亮度事件设备或规则变化，请重新确认范围");
        }
        handle(&DchuLighting(hal), &mut scope.baseline, &info)
    }
}

#[cfg(test)]
#[path = "brightness_event/tests.rs"]
mod tests;
