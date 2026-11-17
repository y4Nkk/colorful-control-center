use crate::{
    hal::{Hal, oem_dchu::Dchu},
    osd,
};
use anyhow::{Context, Result, bail};
use ccc_core::oem_events::CameraReport;

const IDENTITY: &str = "oem-camera-reports-1";

pub(super) fn identity(hal: &Hal) -> Result<String> {
    let _gate = hal.begin_write()?;
    hal.with_dchu(|io| io.read_settings(1, 6, 1))
        .context("摄像头报告保存值不可读取")?;
    Ok(IDENTITY.into())
}

fn require_identity(expected: &str) -> Result<()> {
    if expected != IDENTITY {
        bail!("摄像头反馈授权已变化，请重新确认范围");
    }
    Ok(())
}

fn write_verified(io: &Dchu, value: u8) -> Result<()> {
    io.write_settings(1, 6, &[value])?;
    if io.read_settings(1, 6, 1)? != [value] {
        bail!("摄像头报告保存值回读不一致");
    }
    Ok(())
}

fn synchronize(io: &Dchu, report: CameraReport, expected: &str) -> Result<()> {
    require_identity(expected)?;
    let Some(target) = report.saved_camera() else {
        return Ok(());
    };
    let before = io.read_settings(1, 6, 1)?[0];
    if before == target {
        return Ok(());
    }
    if let Err(error) = write_verified(io, target) {
        return match write_verified(io, before) {
            Ok(()) => Err(error.context("摄像头报告同步失败，原保存值已恢复")),
            Err(restore) => Err(anyhow::anyhow!(
                "摄像头报告同步失败：{error:#}；原保存值恢复失败：{restore:#}"
            )),
        };
    }
    Ok(())
}

fn message(report: CameraReport) -> osd::Message {
    let (title, enabled) = match report {
        CameraReport::Camera(enabled) => ("摄像头", enabled),
        CameraReport::CameraMicrophone(enabled) => ("摄像头与麦克风", enabled),
    };
    let state = if enabled { "已开启" } else { "已关闭" };
    let detail = if report.saved_camera().is_some() {
        format!("原厂报告：{state}；摄像头保存值已同步")
    } else {
        format!("原厂报告：{state}")
    };
    osd::Message::result(title, detail)
}

pub(super) fn handle(hal: &Hal, expected: &str, report: CameraReport) -> Result<osd::Message> {
    require_identity(expected)?;
    let _gate = hal.begin_write()?;
    hal.with_dchu(|io| synchronize(io, report, expected))?;
    Ok(message(report))
}

#[cfg(test)]
#[path = "camera_report/tests.rs"]
mod tests;
