use super::{
    Store,
    geometry::{Rect, SavedPlacement, fit},
    read,
};
use std::sync::{Arc, Mutex};
use tauri::{LogicalSize, Manager, PhysicalSize};

const DEFAULT: LogicalSize<f64> = LogicalSize::new(1100.0, 720.0);
const MINIMUM: LogicalSize<f64> = LogicalSize::new(900.0, 560.0);

fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
    Rect {
        x,
        y,
        size: PhysicalSize::new(width, height),
    }
}

fn saved() -> SavedPlacement {
    SavedPlacement {
        display: "monitor-a".into(),
        offset_x: 100.0,
        offset_y: 50.0,
        width: 1040.0,
        height: 640.0,
    }
}

#[test]
fn first_open_centers_the_scaled_client_and_frame_in_the_work_area() {
    assert_eq!(
        fit(
            rect(0, 60, 1920, 1020),
            1.25,
            PhysicalSize::new(20, 10),
            DEFAULT,
            MINIMUM,
            None,
            false,
        ),
        rect(262, 115, 1395, 910),
    );
}

#[test]
fn restore_keeps_logical_size_and_offset_on_a_repositioned_display() {
    let saved = saved();
    assert!(saved.matches("MONITOR-A"));
    assert_eq!(
        fit(
            rect(-2100, -480, 2100, 1200),
            1.5,
            PhysicalSize::new(20, 10),
            DEFAULT,
            MINIMUM,
            Some(&saved),
            true,
        ),
        rect(-1950, -405, 1580, 970),
    );
}

#[test]
fn missing_display_centers_the_remembered_size_on_primary() {
    let saved = saved();
    assert!(!saved.matches("monitor-b"));
    assert_eq!(
        fit(
            rect(48, 0, 1872, 1080),
            1.0,
            PhysicalSize::new(0, 0),
            DEFAULT,
            MINIMUM,
            Some(&saved),
            false,
        ),
        rect(464, 220, 1040, 640),
    );
}

#[test]
fn restore_fits_after_resolution_changes_and_relaxes_the_minimum_on_small_screens() {
    let saved = SavedPlacement {
        offset_x: 1600.0,
        offset_y: -300.0,
        ..saved()
    };
    let work = rect(-1800, 24, 1800, 976);
    assert_eq!(
        fit(
            work,
            1.25,
            PhysicalSize::new(20, 12),
            DEFAULT,
            MINIMUM,
            Some(&saved),
            true,
        ),
        rect(-1320, 24, 1320, 812),
    );
    let tiny = rect(0, 80, 1280, 720);
    assert_eq!(
        fit(
            tiny,
            2.0,
            PhysicalSize::new(24, 16),
            DEFAULT,
            MINIMUM,
            None,
            false
        ),
        tiny,
    );
}

#[test]
fn a_small_saved_size_grows_to_the_regular_minimum_on_a_larger_display() {
    let saved = SavedPlacement {
        width: 800.0,
        height: 400.0,
        ..saved()
    };
    assert_eq!(
        fit(
            rect(0, 0, 1920, 1040),
            1.0,
            PhysicalSize::new(16, 8),
            DEFAULT,
            MINIMUM,
            Some(&saved),
            true,
        ),
        rect(100, 50, 916, 568),
    );
}

#[test]
fn local_cache_round_trips_and_replaces_only_with_a_normal_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("app").join(super::FILE_NAME);
    let store = Store {
        path,
        latest: Arc::new(Mutex::new(None)),
    };
    assert_eq!(read(&store.path).unwrap(), None);
    store.save().unwrap();
    assert!(!store.path.exists());

    for height in [640.0, 660.0] {
        let placement = SavedPlacement { height, ..saved() };
        *store.latest.lock().unwrap() = Some(placement.clone());
        store.save().unwrap();
        assert_eq!(read(&store.path).unwrap(), Some(placement));
    }

    *store.latest.lock().unwrap() = None;
    store.save().unwrap();
    assert_eq!(read(&store.path).unwrap().unwrap().height, 660.0);
}

#[test]
fn local_cache_rejects_malformed_geometry() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(super::FILE_NAME);
    for contents in [
        r#"{"display":"monitor-a","offsetX":10,"offsetY":0,"width":0,"height":560}"#,
        r#"{"display":"monitor-a","x":10,"y":0,"width":900,"height":560}"#,
        "{",
    ] {
        std::fs::write(&path, contents).unwrap();
        assert!(read(&path).is_err());
    }
}

#[test]
#[ignore = "需要 Windows 桌面和 WebView2"]
fn windows_places_a_hidden_tauri_window_and_restores_the_saved_bounds() {
    use super::{bind_at, native};
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let directory = tempfile::tempdir().unwrap();
    let mut context = tauri::generate_context!();
    let mut config = context.config_mut().app.windows.remove(0);
    assert_eq!(config.label, "main");
    config.visible = false;
    config.focus = false;
    config.url = WebviewUrl::External("about:blank".parse().unwrap());
    config.data_directory = Some(directory.path().join("webview"));
    let app = tauri::Builder::default()
        .any_thread()
        .build(context)
        .unwrap();
    let window = WebviewWindowBuilder::from_config(app.handle(), &config)
        .unwrap()
        .build()
        .unwrap();
    let path = directory.path().join(super::FILE_NAME);
    bind_at(&window, &config, Some(path.clone())).unwrap();
    let hwnd = window.hwnd().unwrap().0;

    let monitor = window.primary_monitor().unwrap().unwrap();
    assert_initial_geometry(&window, &monitor, &config);
    let work = monitor.work_area();
    let first = native::capture(hwnd, false).unwrap();
    assert_visible_geometry_is_saved(&window, &config, &path, &first);
    let saved = read(&path).unwrap().unwrap();

    let missing = SavedPlacement {
        display: "unplugged-display".into(),
        ..saved.clone()
    };
    native::restore(&window, hwnd, &config, Some(&missing)).unwrap();
    let outer = window.outer_size().unwrap();
    let centered = tauri::PhysicalPosition::new(
        work.position.x + ((work.size.width - outer.width) / 2) as i32,
        work.position.y + ((work.size.height - outer.height) / 2) as i32,
    );
    assert_eq!(window.outer_position().unwrap(), centered);
    native::restore(&window, hwnd, &config, Some(&saved)).unwrap();
    assert_eq!(native::capture(hwnd, false).unwrap(), saved);
    assert!(!window.is_visible().unwrap());
    eprintln!(
        "{} 个显示器，主屏缩放 {:.0}%，工作区 {}×{}",
        window.available_monitors().unwrap().len(),
        monitor.scale_factor() * 100.0,
        work.size.width,
        work.size.height
    );
    window.destroy().unwrap();
}

fn assert_visible_geometry_is_saved(
    window: &tauri::WebviewWindow,
    config: &tauri::utils::config::WindowConfig,
    path: &std::path::Path,
    saved: &SavedPlacement,
) {
    use super::{native, save};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_HIDE, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, ShowWindow,
    };

    let hwnd = window.hwnd().unwrap().0;
    unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
    let requested = SavedPlacement {
        width: MINIMUM.width,
        height: MINIMUM.height,
        offset_x: 32.0,
        offset_y: 48.0,
        ..saved.clone()
    };
    native::restore(window, hwnd, config, Some(&requested)).unwrap();
    let actual = native::capture(hwnd, true).unwrap();
    save(window.app_handle());
    assert_eq!(read(path).unwrap().unwrap(), actual);

    unsafe { ShowWindow(hwnd, SW_SHOWMINNOACTIVE) };
    assert!(native::capture(hwnd, false).is_none());
    save(window.app_handle());
    assert_eq!(read(path).unwrap().unwrap(), actual);
    unsafe {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        ShowWindow(hwnd, SW_HIDE);
    }
}

fn assert_initial_geometry(
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
    config: &tauri::utils::config::WindowConfig,
) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MINMAXINFO, SendMessageW, WM_GETMINMAXINFO};

    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let outer = window.outer_size().unwrap();
    let inner = window.inner_size().unwrap();
    let position = window.outer_position().unwrap();
    let frame = PhysicalSize::new(outer.width - inner.width, outer.height - inner.height);
    let wanted = LogicalSize::new(config.width, config.height).to_physical::<u32>(scale);
    assert_eq!(
        outer.width,
        (wanted.width + frame.width).min(work.size.width)
    );
    assert_eq!(
        outer.height,
        (wanted.height + frame.height).min(work.size.height)
    );
    assert_eq!(
        position.x,
        work.position.x + ((work.size.width - outer.width) / 2) as i32
    );
    assert_eq!(
        position.y,
        work.position.y + ((work.size.height - outer.height) / 2) as i32
    );
    assert!(!window.is_visible().unwrap());

    let mut limits = MINMAXINFO::default();
    unsafe {
        SendMessageW(
            window.hwnd().unwrap().0,
            WM_GETMINMAXINFO,
            0,
            &mut limits as *mut MINMAXINFO as isize,
        )
    };
    let minimum = LogicalSize::new(config.min_width.unwrap(), config.min_height.unwrap())
        .to_physical::<u32>(scale);
    assert_eq!(
        limits.ptMinTrackSize.x as u32,
        (minimum.width + frame.width).min(work.size.width)
    );
    assert_eq!(
        limits.ptMinTrackSize.y as u32,
        (minimum.height + frame.height).min(work.size.height)
    );
}
