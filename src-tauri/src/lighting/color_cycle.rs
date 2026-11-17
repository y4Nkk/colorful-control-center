use super::*;
use anyhow::{Context, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
#[path = "color_cycle/tests.rs"]
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Info {
    pub keyboard_type: u8,
    pub customer_id: u8,
    pub monster_manufacturer: bool,
    pub logo: bool,
    pub light_bar: bool,
}
impl Info {
    fn validate(&self) -> Result<()> {
        if !matches!(self.keyboard_type, 6 | 22 | 38) {
            bail!("此键盘未提供原厂颜色循环");
        }
        Ok(())
    }
    fn maximum(&self) -> u8 {
        if self.customer_id == 0 && self.monster_manufacturer {
            16
        } else {
            15
        }
    }
    fn identity(&self) -> Result<String> {
        self.validate()?;
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
    fn next(&self, current: u8) -> Result<Choice> {
        self.validate()?;
        let index = if current == self.maximum() {
            1
        } else {
            current.wrapping_add(1)
        };
        if index > self.maximum() {
            bail!("原厂颜色循环索引越界，请先恢复已知配置");
        }
        let [r, g, b] = COLORS[usize::from(index)];
        Ok(Choice {
            index,
            color: Rgb { r, g, b },
        })
    }
}
pub(crate) struct Choice {
    pub index: u8,
    pub color: Rgb,
}
impl Choice {
    pub(crate) fn summary(&self) -> String {
        format!(
            "第{}色 · #{:02X}{:02X}{:02X}，灯效设置保留",
            self.index, self.color.r, self.color.g, self.color.b
        )
    }
}
const COLORS: [[u8; 3]; 17] = [
    [0, 0, 255],
    [0, 0, 200],
    [0, 255, 0],
    [255, 0, 0],
    [0, 255, 200],
    [0, 127, 200],
    [0, 255, 127],
    [255, 0, 200],
    [127, 0, 200],
    [255, 0, 127],
    [255, 255, 0],
    [127, 255, 200],
    [255, 127, 0],
    [63, 255, 0],
    [255, 127, 63],
    [63, 127, 127],
    [223, 240, 124],
];

pub(super) fn native_info(hal: &Hal, raw_type: u8) -> Result<Info> {
    if !matches!(raw_type, 6 | 22 | 38) {
        bail!("此键盘未提供原厂颜色循环");
    }
    let (caps, customer_id) =
        hal.with_dchu(|io| Ok((options::native::capabilities(io)?, io.read_package(17)?[2])))?;
    if caps.keyboard_type != raw_type || !caps.peripheral_support_known {
        bail!("键盘或外围灯能力未知，请重新读取");
    }
    let info = Info {
        keyboard_type: raw_type,
        customer_id,
        monster_manufacturer: customer_id == 0 && hal.manufacturer()?.contains("MONSTER"),
        logo: caps.logo,
        light_bar: caps.light_bar,
    };
    info.validate()?;
    Ok(info)
}
fn words(info: &Info, block: &SavedBlock, color: Rgb) -> Vec<u32> {
    let mut values: Vec<_> = (0..3)
        .map(|zone| encode_zone_color(0xF0 + zone, color, Some(info.keyboard_type)))
        .collect();
    if info.logo {
        values.push(encode_zone_color(
            0xF6,
            block.color(idx(101)),
            Some(info.keyboard_type),
        ));
    }
    if info.light_bar {
        values.push(encode_zone_color(
            0xF3,
            block.color(idx(105)),
            Some(info.keyboard_type),
        ));
    }
    values
}
fn apply(io: &impl LightingIo, initial: &mut Option<Baseline>, info: &Info) -> Result<Choice> {
    info.validate()?;
    let mut before = transaction::capture(io, KeyboardFamily::ZoneRgb, Some(info.keyboard_type))
        .map_err(|e| anyhow!(e.message))?;
    transaction::require_baseline(initial, &before).map_err(|e| anyhow!(e.message))?;
    let cursor_before = io.cycle_index()?;
    let choice = info.next(cursor_before)?;
    if io.cycle_info(info.keyboard_type)? != *info {
        bail!("原厂调色板或外围能力变化，请重新预览");
    }
    let block = SavedBlock::from_bytes(before.block.clone())?;
    if io.read_saved_block()?.bytes != before.block || io.cycle_index()? != cursor_before {
        bail!("键盘颜色或索引在准备期间变化");
    }
    initial.get_or_insert_with(|| before.clone());
    let result = (|| {
        for word in words(info, &block, choice.color) {
            io.write_word(CMD_RGB, word)?;
        }
        io.apply_saved(&[Patch::new(
            81,
            [choice.color.r, choice.color.g, choice.color.b],
        )])?;
        before.cycle_index = Some(cursor_before);
        initial
            .as_mut()
            .context("颜色循环首次记录缺失")?
            .cycle_index
            .get_or_insert(cursor_before);
        io.apply_saved(&[Patch::new(127, [choice.index])])?;
        let after = io.read_saved_block()?;
        let mut expected = before.block.clone();
        expected[idx(81)..idx(81) + 3].copy_from_slice(&[
            choice.color.r,
            choice.color.g,
            choice.color.b,
        ]);
        if after.bytes != expected || io.cycle_index()? != choice.index {
            bail!("颜色保存或循环索引回读不一致");
        }
        Ok(())
    })();
    if let Err(error) = result {
        bail!(ccc_core::msr::with_recovery(
            format!("键盘颜色循环失败：{error:#}"),
            transaction::restore_snapshot(io, &before).map_err(|e| format!("{e:#}"))
        ));
    }
    Ok(choice)
}
impl LightingController {
    pub(crate) fn color_cycle_identity(&self, hal: &Hal) -> Result<String> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let target = scope.capture(hal)?.context("键盘灯保存状态未知")?;
        let io = DchuLighting(hal);
        let info = io.cycle_info(target.keyboard_type)?;
        info.next(io.cycle_index()?)?;
        let before = transaction::capture(&io, KeyboardFamily::ZoneRgb, Some(info.keyboard_type))
            .map_err(|e| anyhow!(e.message))?;
        transaction::require_baseline(&scope.baseline, &before).map_err(|e| anyhow!(e.message))?;
        info.identity()
    }
    pub(crate) fn cycle_color(&self, hal: &Hal, expected: &str) -> Result<Choice> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = hal.with_dchu(|d| Ok(build_profile(d, None)))?;
        let info = DchuLighting(hal).cycle_info(profile.detected_type.context("键盘类型未知")?)?;
        if info.identity()? != expected {
            bail!("颜色循环设备或规则变化，请重新确认授权");
        }
        apply(&DchuLighting(hal), &mut scope.baseline, &info)
    }
}
