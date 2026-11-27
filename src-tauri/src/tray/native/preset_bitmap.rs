use anyhow::{Context, Result};
use std::{io, ptr::null_mut};
use windows_sys::Win32::{
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, HBITMAP, SelectObject,
    },
    UI::WindowsAndMessaging::{
        CreateIconFromResourceEx, DI_NORMAL, DestroyIcon, DrawIconEx, GetSystemMetrics, HICON,
        SM_CXSMICON,
    },
};

/// 菜单项位图会顶掉系统对勾，所以对勾与档位图标合成为勾选位图。
const CHECK: &[u8] = include_bytes!("../../../icons/presets/check.png");

pub(super) struct PresetBitmap(pub(super) HBITMAP);

struct Icon(HICON);

impl Drop for Icon {
    fn drop(&mut self) {
        unsafe { DestroyIcon(self.0) };
    }
}

impl PresetBitmap {
    /// 同一档位的（未勾选，已勾选）位图，依次为「留白或对勾｜图标」。
    /// 两张等宽，图标列因此对齐。没有专用图形的档位返回 None。
    pub(super) fn pair(mode: u8) -> Result<Option<(Self, Self)>> {
        let art: &[u8] = match mode {
            0 => include_bytes!("../../../icons/presets/quiet.png"),
            1 => include_bytes!("../../../icons/presets/eco.png"),
            2 => include_bytes!("../../../icons/presets/performance.png"),
            3 => include_bytes!("../../../icons/presets/entertainment.png"),
            _ => return Ok(None),
        };
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
        let (art, check) = (load(art, size)?, load(CHECK, size)?);
        Ok(Some((
            paint(&[None, Some(&art)], size)?,
            paint(&[Some(&check), Some(&art)], size)?,
        )))
    }
}

fn load(bytes: &[u8], size: i32) -> Result<Icon> {
    // CreateIconFromResourceEx 接受 PNG，但要求缓冲区按 DWORD 对齐。
    let mut image = vec![0u32; bytes.len().div_ceil(4)];
    let icon = unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), image.as_mut_ptr().cast(), bytes.len());
        CreateIconFromResourceEx(
            image.as_ptr().cast(),
            bytes.len() as u32,
            1,
            0x00030000,
            size,
            size,
            0,
        )
    };
    if icon.is_null() {
        return Err(io::Error::last_os_error()).context("加载预设菜单图标");
    }
    Ok(Icon(icon))
}

/// 按格横排绘制图标，空格留白；每格边长为 size。
fn paint(cells: &[Option<&Icon>], size: i32) -> Result<PresetBitmap> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size * cells.len() as i32,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = std::ptr::null_mut();
    let handle = unsafe {
        CreateDIBSection(
            null_mut(),
            &info,
            DIB_RGB_COLORS,
            &mut pixels,
            null_mut(),
            0,
        )
    };
    if handle.is_null() {
        return Err(io::Error::last_os_error()).context("创建预设菜单位图");
    }
    let bitmap = PresetBitmap(handle);
    let dc = unsafe { CreateCompatibleDC(null_mut()) };
    if dc.is_null() {
        return Err(io::Error::last_os_error()).context("创建设备上下文");
    }
    // 透明底上的图标绘制保留系统菜单所需的预乘 alpha。
    let drawn = unsafe {
        pixels
            .cast::<u32>()
            .write_bytes(0, (size * size) as usize * cells.len());
        let previous = SelectObject(dc, bitmap.0);
        let result = if previous.is_null() {
            Err(io::Error::last_os_error())
        } else {
            let result = (|| {
                for (index, icon) in cells.iter().enumerate() {
                    if let Some(icon) = icon
                        && DrawIconEx(
                            dc,
                            index as i32 * size,
                            0,
                            icon.0,
                            size,
                            size,
                            0,
                            null_mut(),
                            DI_NORMAL,
                        ) == 0
                    {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            })();
            SelectObject(dc, previous);
            result
        };
        DeleteDC(dc);
        result
    };
    drawn.context("绘制预设菜单位图")?;
    Ok(bitmap)
}

impl Drop for PresetBitmap {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.0) };
    }
}
