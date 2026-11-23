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

pub(super) struct PresetBitmap(pub(super) HBITMAP);

impl PresetBitmap {
    pub(super) fn load(mode: u8) -> Result<Option<Self>> {
        let bytes: &[u8] = match mode {
            0 | 4 => include_bytes!("../../../icons/presets/quiet.png"),
            1 => include_bytes!("../../../icons/presets/eco.png"),
            2 => include_bytes!("../../../icons/presets/performance.png"),
            3 => include_bytes!("../../../icons/presets/entertainment.png"),
            _ => return Ok(None),
        };
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
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
        let bitmap = Self::draw(icon, size);
        unsafe { DestroyIcon(icon) };
        bitmap.map(Some)
    }

    fn draw(icon: HICON, size: i32) -> Result<Self> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = null_mut();
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
        let bitmap = Self(handle);
        let dc = unsafe { CreateCompatibleDC(null_mut()) };
        if dc.is_null() {
            return Err(io::Error::last_os_error()).context("创建设备上下文");
        }
        // 透明底上的图标绘制保留系统菜单所需的预乘 alpha。
        let drawn = unsafe {
            pixels
                .cast::<u32>()
                .write_bytes(0, size as usize * size as usize);
            let previous = SelectObject(dc, bitmap.0);
            let result = if previous.is_null() {
                Err(io::Error::last_os_error())
            } else {
                let drawn = DrawIconEx(dc, 0, 0, icon, size, size, 0, null_mut(), DI_NORMAL);
                let result = if drawn == 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                };
                SelectObject(dc, previous);
                result
            };
            DeleteDC(dc);
            result
        };
        drawn.context("绘制预设菜单位图")?;
        Ok(bitmap)
    }
}

impl Drop for PresetBitmap {
    fn drop(&mut self) {
        unsafe { DeleteObject(self.0) };
    }
}
