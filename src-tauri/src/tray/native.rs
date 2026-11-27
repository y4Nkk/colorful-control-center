#[path = "native/menu.rs"]
mod menu;
#[path = "native/preset_bitmap.rs"]
mod preset_bitmap;

use anyhow::{Context as _, Result, bail};
use std::{
    cell::OnceCell,
    io,
    ptr::{null, null_mut},
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Shell::{
            NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
            NIM_SETFOCUS, NIM_SETVERSION, NIN_SELECT, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
            NOTIFYICONIDENTIFIER, Shell_NotifyIconGetRect, Shell_NotifyIconW,
        },
        WindowsAndMessaging::*,
    },
};

const CALLBACK: u32 = WM_APP + 1;
const REFRESH_TIP: u32 = WM_APP + 2;
const ICON_ID: u32 = 1;
const RETRY_ICON: usize = 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const READING_HARDWARE: &str = "正在读取硬件状态";

pub(super) enum Action {
    Quit,
    Show,
    Profile(String),
    Cooling,
    Control(Control),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FanAction {
    Auto,
    FullSpeed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Control {
    Preset(u8),
    Fan(FanAction),
}

pub(super) struct Presets {
    pub current: u8,
    pub label: &'static str,
    pub choices: Vec<(u8, &'static str)>,
    pub blocked: Option<String>,
}

pub(super) struct Fans {
    pub current: u8,
    pub label: &'static str,
    pub active: Option<FanAction>,
    pub choices: Vec<FanAction>,
    pub blocked: Option<String>,
}

pub(super) struct Controls {
    pub presets: Result<Presets, String>,
    pub fans: Result<Fans, String>,
}

pub(super) struct MenuData {
    pub controls: Controls,
    pub profiles: Option<Vec<String>>,
}

#[derive(Default)]
struct State {
    control_labels: Option<(&'static str, &'static str)>,
    temperatures: String,
    busy: bool,
}

impl State {
    fn tooltip(&self) -> String {
        let Some((preset, fan)) = self.control_labels else {
            return format!("Colorful Control Center\n{READING_HARDWARE}");
        };
        format!(
            "Colorful Control Center\n预设 {preset} · 风扇 {fan}{}",
            self.temperatures
        )
    }
}

type SharedState = Arc<Mutex<State>>;

struct Context {
    state: SharedState,
    dispatch: Box<dyn Fn(Action) + Send>,
    read_menu: Box<dyn Fn() -> MenuData + Send>,
    closing: fn() -> bool,
    taskbar_created: u32,
    icon: HICON,
}

thread_local! {
    static CONTEXT: OnceCell<Context> = const { OnceCell::new() };
}

pub(super) struct Tray {
    state: SharedState,
    hwnd: usize,
    worker: Mutex<Option<JoinHandle<Result<()>>>>,
}

impl Tray {
    pub(super) fn start(
        resource: Option<u16>,
        dispatch: impl Fn(Action) + Send + 'static,
        read_menu: impl Fn() -> MenuData + Send + 'static,
        closing: fn() -> bool,
    ) -> Result<Self> {
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        let (ready, wait) = mpsc::sync_channel(1);
        let worker =
            thread::Builder::new()
                .name("windows-tray".into())
                .spawn(move || {
                    match create(
                        shared,
                        Box::new(dispatch),
                        Box::new(read_menu),
                        closing,
                        resource,
                    ) {
                        Ok(hwnd) => {
                            if ready.send(Ok(hwnd as usize)).is_err() {
                                unsafe { DestroyWindow(hwnd) };
                                return Ok(());
                            }
                            run(hwnd)
                        }
                        Err(error) => {
                            let _ = ready.send(Err(error));
                            Ok(())
                        }
                    }
                })?;
        Ok(Self {
            state,
            hwnd: wait.recv().context("托盘线程未就绪")??,
            worker: Mutex::new(Some(worker)),
        })
    }

    pub(super) fn set_control_labels(&self, controls: &Controls) {
        let preset = controls
            .presets
            .as_ref()
            .map_or("不可用", |state| state.label);
        let fan = controls.fans.as_ref().map_or("不可用", |state| state.label);
        self.update_tooltip(|state| state.control_labels = Some((preset, fan)));
    }

    pub(super) fn set_telemetry(
        &self,
        preset: &'static str,
        fan: &'static str,
        temperatures: String,
    ) {
        self.update_tooltip(|state| {
            state.control_labels = Some((preset, fan));
            state.temperatures = temperatures;
        });
    }

    pub(super) fn set_temperatures(&self, temperatures: String) {
        self.update_tooltip(|state| state.temperatures = temperatures);
    }

    fn update_tooltip(&self, update: impl FnOnce(&mut State)) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let previous = state.tooltip();
        update(&mut state);
        if state.tooltip() != previous {
            unsafe { PostMessageW(self.window(), REFRESH_TIP, 0, 0) };
        }
    }

    pub(super) fn begin_control(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        !std::mem::replace(&mut state.busy, true)
    }

    pub(super) fn end_control(&self) {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .busy = false;
    }

    pub(super) fn window(&self) -> HWND {
        self.hwnd as HWND
    }

    pub(super) fn stop(&self) -> Result<()> {
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(worker) = worker {
            unsafe { PostMessageW(self.window(), WM_CANCELMODE, 0, 0) };
            if !worker.is_finished()
                && unsafe { PostMessageW(self.window(), WM_CLOSE, 0, 0) } == 0
                && !worker.is_finished()
            {
                return Err(io::Error::last_os_error()).context("发送托盘退出消息");
            }
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("托盘线程异常"))??;
        }
        Ok(())
    }
}

fn create(
    state: SharedState,
    dispatch: Box<dyn Fn(Action) + Send>,
    read_menu: Box<dyn Fn() -> MenuData + Send>,
    closing: fn() -> bool,
    resource: Option<u16>,
) -> Result<HWND> {
    let module = unsafe { GetModuleHandleW(null()) };
    let icon = if let Some(resource) = resource {
        let icon = unsafe { LoadIconW(module, resource as usize as *const u16) };
        if icon.is_null() {
            return Err(io::Error::last_os_error()).context("加载托盘图标");
        }
        icon
    } else {
        null_mut()
    };
    let taskbar_created = unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
    if taskbar_created == 0 {
        return Err(io::Error::last_os_error()).context("注册任务栏消息");
    }
    CONTEXT.with(|slot| {
        let _ = slot.set(Context {
            state,
            dispatch,
            read_menu,
            closing,
            taskbar_created,
            icon,
        });
    });
    let name = wide("ColorfulControlCenter.Tray");
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: module,
        lpszClassName: name.as_ptr(),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(io::Error::last_os_error()).context("注册托盘窗口类");
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            name.as_ptr(),
            null(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            module,
            null(),
        )
    };
    if hwnd.is_null() {
        return Err(io::Error::last_os_error()).context("创建托盘窗口");
    }
    if resource.is_some() {
        unsafe {
            ChangeWindowMessageFilterEx(hwnd, CALLBACK, MSGFLT_ALLOW, null_mut());
            ChangeWindowMessageFilterEx(hwnd, taskbar_created, MSGFLT_ALLOW, null_mut());
        }
    }
    Ok(hwnd)
}

fn run(hwnd: HWND) -> Result<()> {
    CONTEXT.with(|slot| {
        if let Some(context) = slot.get() {
            add_icon(hwnd, context);
        }
    });
    let mut message = MSG::default();
    loop {
        match unsafe { GetMessageW(&mut message, null_mut(), 0, 0) } {
            -1 => {
                let error = io::Error::last_os_error();
                unsafe { DestroyWindow(hwnd) };
                bail!("托盘消息循环：{error}");
            }
            0 => return Ok(()),
            _ => unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            },
        }
    }
}

fn icon_data(hwnd: HWND, context: &Context) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: CALLBACK,
        hIcon: context.icon,
        ..Default::default()
    };
    let state = context
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let capacity = data.szTip.len() - 1;
    for (unit, source) in data.szTip[..capacity]
        .iter_mut()
        .zip(state.tooltip().encode_utf16())
    {
        *unit = source;
    }
    data
}

fn add_icon(hwnd: HWND, context: &Context) {
    if context.icon.is_null() {
        return;
    }
    let mut data = icon_data(hwnd, context);
    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    unsafe {
        if Shell_NotifyIconW(NIM_ADD, &data) != 0 {
            if Shell_NotifyIconW(NIM_SETVERSION, &data) == 0 {
                Shell_NotifyIconW(NIM_DELETE, &data);
                SetTimer(hwnd, RETRY_ICON, 1000, None);
                return;
            }
            KillTimer(hwnd, RETRY_ICON);
        } else {
            SetTimer(hwnd, RETRY_ICON, 1000, None);
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    CONTEXT.with(|slot| {
        if let Some(context) = slot.get() {
            match message {
                CALLBACK => {
                    notify(hwnd, lparam, context);
                    return 0;
                }
                WM_ENDSESSION if wparam != 0 => (context.dispatch)(Action::Quit),
                WM_CLOSE => unsafe {
                    EndMenu();
                    DestroyWindow(hwnd);
                    return 0;
                },
                WM_DESTROY => unsafe {
                    if !context.icon.is_null() {
                        Shell_NotifyIconW(NIM_DELETE, &icon_data(hwnd, context));
                    }
                    PostQuitMessage(0);
                    return 0;
                },
                REFRESH_TIP if !context.icon.is_null() => {
                    let mut data = icon_data(hwnd, context);
                    data.uFlags = NIF_TIP | NIF_SHOWTIP;
                    unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
                }
                WM_TIMER if wparam == RETRY_ICON => add_icon(hwnd, context),
                msg if msg == context.taskbar_created => add_icon(hwnd, context),
                _ => (),
            }
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    })
}

fn notify(hwnd: HWND, event: LPARAM, context: &Context) {
    if ((event as u32) >> 16) != ICON_ID {
        return;
    }
    let action = match (event as u32) & 0xffff {
        NIN_SELECT | NIN_KEYSELECT => Some(Action::Show),
        WM_CONTEXTMENU => {
            let selected = popup(hwnd, context);
            if selected.is_none() {
                unsafe { Shell_NotifyIconW(NIM_SETFOCUS, &icon_data(hwnd, context)) };
            }
            selected
        }
        _ => None,
    };
    if let Some(action) = action {
        (context.dispatch)(action);
    }
}

fn popup(hwnd: HWND, context: &Context) -> Option<Action> {
    let state = if (context.closing)() {
        menu::MenuState::default()
    } else {
        let MenuData { controls, profiles } = (context.read_menu)();
        let busy = context
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .busy;
        menu::MenuState {
            profiles,
            controls: Some(controls),
            busy,
        }
    };
    match menu::popup(hwnd, menu_anchor(hwnd), &state, (context.closing)()) {
        Ok(action) => action,
        Err(error) => {
            log::error!("[tray] 打开菜单：{error:#}");
            None
        }
    }
}

fn menu_anchor(hwnd: HWND) -> POINT {
    let icon = NOTIFYICONIDENTIFIER {
        cbSize: size_of::<NOTIFYICONIDENTIFIER>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        ..Default::default()
    };
    let mut rect = RECT::default();
    if unsafe { Shell_NotifyIconGetRect(&icon, &mut rect) } >= 0 {
        return POINT {
            x: (rect.left + rect.right) / 2,
            y: (rect.top + rect.bottom) / 2,
        };
    }
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor) };
    cursor
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

pub(super) fn confirm(title: &str, detail: &str) -> Result<bool> {
    let result = unsafe {
        MessageBoxW(
            null_mut(),
            wide(detail).as_ptr(),
            wide(title).as_ptr(),
            MB_OKCANCEL | MB_DEFBUTTON2 | MB_ICONWARNING | MB_SETFOREGROUND,
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error()).context("显示托盘确认");
    }
    Ok(result == IDOK)
}
