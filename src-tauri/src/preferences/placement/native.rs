use super::{
    LatestPlacement,
    geometry::{Rect, SavedPlacement, fit},
};
use anyhow::{Context, Result, bail, ensure};
use std::{io, mem::size_of, ptr::null_mut, sync::Arc};
use tauri::{LogicalSize, Monitor, PhysicalSize, WebviewWindow, utils::config::WindowConfig};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICEW, EnumDisplayDevicesW, GetMonitorInfoW,
        MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromWindow,
    },
    UI::{
        HiDpi::GetDpiForWindow,
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            EDD_GET_DEVICE_INTERFACE_NAME, GetWindowInfo, IsIconic, IsWindowVisible, IsZoomed,
            MINMAXINFO, SWP_NOACTIVATE, SWP_NOSENDCHANGING, SWP_NOZORDER, SetWindowPos, WINDOWINFO,
            WM_GETMINMAXINFO, WM_NCDESTROY, WM_WINDOWPOSCHANGED,
        },
    },
};

pub(super) fn restore(
    window: &WebviewWindow,
    hwnd: HWND,
    config: &WindowConfig,
    saved: Option<&SavedPlacement>,
) -> Result<()> {
    let remembered = match saved {
        Some(saved) => window.available_monitors()?.into_iter().find(|monitor| {
            monitor
                .name()
                .is_some_and(|name| saved.matches(&display_id(name)))
        }),
        None => None,
    };
    let restore_offset = remembered.is_some();
    let monitor = match remembered {
        Some(monitor) => monitor,
        None => window.primary_monitor()?.context("没有可用的主显示器")?,
    };
    let work = work_area(&monitor);
    ensure!(
        work.size.width > 0 && work.size.height > 0,
        "显示器工作区无效"
    );
    let bounds = |frame| {
        fit(
            work,
            monitor.scale_factor(),
            frame,
            LogicalSize::new(config.width, config.height),
            LogicalSize::new(
                config.min_width.unwrap_or(0.0),
                config.min_height.unwrap_or(0.0),
            ),
            saved,
            restore_offset,
        )
    };
    let already_on_target = window
        .current_monitor()?
        .is_some_and(|current| current.position() == monitor.position());
    if !already_on_target {
        // 先迁移隐藏窗口，让 Windows 应用目标显示器的 DPI 和边框。
        set_bounds(hwnd, bounds(PhysicalSize::new(0, 0)))?;
    }
    let info = window_info(hwnd).context("读取主窗口边框失败")?;
    set_bounds(hwnd, bounds(frame_size(&info)))
}

pub(super) fn capture(hwnd: HWND, visible_only: bool) -> Option<SavedPlacement> {
    if unsafe {
        IsIconic(hwnd) != 0 || IsZoomed(hwnd) != 0 || (visible_only && IsWindowVisible(hwnd) == 0)
    } {
        return None;
    }
    let info = window_info(hwnd)?;
    let monitor = monitor_info(hwnd)?;
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        return None;
    }
    let scale = f64::from(dpi) / 96.0;
    let work = monitor.monitorInfo.rcWork;
    let client = info.rcClient;
    let saved = SavedPlacement {
        display: display_id(&wide_text(&monitor.szDevice)),
        offset_x: f64::from(info.rcWindow.left - work.left) / scale,
        offset_y: f64::from(info.rcWindow.top - work.top) / scale,
        width: f64::from(client.right - client.left) / scale,
        height: f64::from(client.bottom - client.top) / scale,
    };
    saved.valid().then_some(saved)
}

pub(super) fn track(hwnd: HWND, latest: &Arc<LatestPlacement>) -> Result<()> {
    let reference = Arc::into_raw(Arc::clone(latest));
    if unsafe { SetWindowSubclass(hwnd, Some(placement_proc), 0, reference as usize) } == 0 {
        unsafe { drop(Arc::from_raw(reference)) };
        bail!("无法安装主窗口位置与缩放处理器");
    }
    Ok(())
}

unsafe extern "system" fn placement_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    reference: usize,
) -> LRESULT {
    let reference = reference as *const LatestPlacement;
    // 窗口销毁时回收子类持有的 Arc。转发消息前使用快照引用，避免窗口过程重入。
    if message == WM_NCDESTROY {
        unsafe {
            RemoveWindowSubclass(hwnd, Some(placement_proc), id);
            drop(Arc::from_raw(reference));
        }
    } else if message == WM_WINDOWPOSCHANGED
        && let Some(saved) = capture(hwnd, true)
        && let Ok(mut latest) = unsafe { &*reference }.lock()
    {
        *latest = Some(saved);
    }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if message == WM_GETMINMAXINFO
        && lparam != 0
        && let Some(monitor) = monitor_info(hwnd)
    {
        let work = size_of_rect(monitor.monitorInfo.rcWork);
        if work.width > 0 && work.height > 0 {
            let frame = window_info(hwnd).map_or(PhysicalSize::new(0, 0), |info| frame_size(&info));
            let minimum = unsafe { &mut (*(lparam as *mut MINMAXINFO)).ptMinTrackSize };
            // Tao 的无标题栏下限是内容尺寸。Windows 追踪的是含边框尺寸。
            minimum.x = minimum.x.saturating_add(frame.width).min(work.width);
            minimum.y = minimum.y.saturating_add(frame.height).min(work.height);
        }
    }
    result
}

fn set_bounds(hwnd: HWND, rect: Rect) -> Result<()> {
    let ok = unsafe {
        SetWindowPos(
            hwnd,
            null_mut(),
            rect.x,
            rect.y,
            rect.size.width,
            rect.size.height,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSENDCHANGING,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error()).context("设置主窗口位置和尺寸失败");
    }
    Ok(())
}

fn display_id(name: &str) -> String {
    let wide: Vec<_> = name.encode_utf16().chain([0]).collect();
    for index in 0.. {
        let mut display = DISPLAY_DEVICEW {
            cb: size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        if unsafe {
            EnumDisplayDevicesW(
                wide.as_ptr(),
                index,
                &mut display,
                EDD_GET_DEVICE_INTERFACE_NAME,
            )
        } == 0
        {
            break;
        }
        if display.StateFlags & DISPLAY_DEVICE_ACTIVE != 0 && display.DeviceID[0] != 0 {
            return wide_text(&display.DeviceID);
        }
    }
    name.to_owned()
}

fn monitor_info(hwnd: HWND) -> Option<MONITORINFOEXW> {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    let ok = unsafe {
        GetMonitorInfoW(
            monitor,
            (&mut info as *mut MONITORINFOEXW).cast::<MONITORINFO>(),
        )
    };
    (ok != 0).then_some(info)
}

fn window_info(hwnd: HWND) -> Option<WINDOWINFO> {
    let mut info = WINDOWINFO {
        cbSize: size_of::<WINDOWINFO>() as u32,
        ..Default::default()
    };
    (unsafe { GetWindowInfo(hwnd, &mut info) } != 0).then_some(info)
}

fn work_area(monitor: &Monitor) -> Rect {
    let work = monitor.work_area();
    Rect {
        x: work.position.x,
        y: work.position.y,
        size: PhysicalSize::new(work.size.width as i32, work.size.height as i32),
    }
}

fn frame_size(info: &WINDOWINFO) -> PhysicalSize<i32> {
    let window = size_of_rect(info.rcWindow);
    let client = size_of_rect(info.rcClient);
    PhysicalSize::new(
        (window.width - client.width).max(0),
        (window.height - client.height).max(0),
    )
}

fn size_of_rect(rect: RECT) -> PhysicalSize<i32> {
    PhysicalSize::new(rect.right - rect.left, rect.bottom - rect.top)
}

fn wide_text(raw: &[u16]) -> String {
    String::from_utf16_lossy(&raw[..raw.iter().position(|&unit| unit == 0).unwrap_or(raw.len())])
}
