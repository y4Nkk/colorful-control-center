use anyhow::{Result, bail, ensure};
use serde::Serialize;
use std::{mem::size_of, ptr::null_mut};
use windows_sys::Win32::{
    Devices::Display::*,
    Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS},
};

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct EmbeddedPanel {
    pub device: String,
    identity: String,
}

pub(crate) fn embedded_panel() -> Result<EmbeddedPanel> {
    let paths = active_paths()?;
    let path = unique_embedded_path(&paths)?;
    let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
            adapterId: path.sourceInfo.adapterId,
            id: path.sourceInfo.id,
        },
        ..Default::default()
    };
    let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
            adapterId: path.targetInfo.adapterId,
            id: path.targetInfo.id,
        },
        ..Default::default()
    };
    let source_status = unsafe { DisplayConfigGetDeviceInfo(&mut source.header) };
    let target_status = unsafe { DisplayConfigGetDeviceInfo(&mut target.header) };
    ensure!(
        source_status == 0 && target_status == 0,
        "无法读取内屏身份（{source_status}, {target_status}）"
    );
    let device = super::text(&source.viewGdiDeviceName);
    let identity = super::text(&target.monitorDevicePath);
    ensure!(
        !device.is_empty() && !identity.is_empty(),
        "Windows 未报告完整的内屏身份"
    );
    Ok(EmbeddedPanel { device, identity })
}

fn unique_embedded_path(paths: &[DISPLAYCONFIG_PATH_INFO]) -> Result<&DISPLAYCONFIG_PATH_INFO> {
    let mut panels = paths.iter().filter(|path| {
        path.targetInfo.targetAvailable != 0
            && matches!(
                path.targetInfo.outputTechnology,
                DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL
                    | DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS
                    | DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED
                    | DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED
            )
    });
    let Some(panel) = panels.next() else {
        bail!("未识别到活动内屏")
    };
    ensure!(panels.next().is_none(), "活动内屏身份不唯一");
    let source = panel.sourceInfo;
    let sharing_source = paths.iter().filter(|path| {
        let other = path.sourceInfo;
        other.id == source.id
            && other.adapterId.HighPart == source.adapterId.HighPart
            && other.adapterId.LowPart == source.adapterId.LowPart
    });
    ensure!(
        sharing_source.count() == 1,
        "内屏处于复制显示模式，无法单独设置刷新率"
    );
    Ok(panel)
}

fn active_paths() -> Result<Vec<DISPLAYCONFIG_PATH_INFO>> {
    for _ in 0..3 {
        let (mut paths_count, mut modes_count) = (0, 0);
        let status = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut paths_count, &mut modes_count)
        };
        ensure!(
            status == ERROR_SUCCESS,
            "无法读取活动显示路径数量（{status}）"
        );
        if paths_count == 0 {
            return Ok(Vec::new());
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); paths_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); modes_count as usize];
        let status = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut paths_count,
                paths.as_mut_ptr(),
                &mut modes_count,
                modes.as_mut_ptr(),
                null_mut(),
            )
        };
        if status == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        ensure!(status == ERROR_SUCCESS, "无法读取活动显示路径（{status}）");
        paths.truncate(paths_count as usize);
        return Ok(paths);
    }
    bail!("显示拓扑正在变化，请重新预览")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(
        source: u32,
        technology: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
    ) -> DISPLAYCONFIG_PATH_INFO {
        let mut path = DISPLAYCONFIG_PATH_INFO::default();
        path.sourceInfo.id = source;
        path.targetInfo.outputTechnology = technology;
        path.targetInfo.targetAvailable = 1;
        path
    }

    #[test]
    fn selects_only_a_unique_uncloned_embedded_panel() {
        let panel = path(1, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED);
        let external = path(0, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI);
        assert_eq!(
            unique_embedded_path(&[external, panel])
                .unwrap()
                .sourceInfo
                .id,
            1
        );
        assert!(unique_embedded_path(&[external]).is_err());
        assert!(
            unique_embedded_path(&[panel, path(2, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL)])
                .is_err()
        );
        assert!(
            unique_embedded_path(&[panel, path(1, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI)]).is_err()
        );
    }
}
