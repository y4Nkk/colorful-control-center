use super::preset_bitmap::PresetBitmap;
use super::{Action, Control, Controls, FanAction, READING_HARDWARE, wide};
use anyhow::{Context, Result};
use std::{
    io,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{HWND, POINT},
    UI::WindowsAndMessaging::*,
};

const SHOW: usize = 1;
const QUIT: usize = 2;
const COOLING: usize = 3;
const FAN_AUTO: usize = 4;
const FAN_FULL_SPEED: usize = 5;
const FIRST_PRESET: usize = 6;
const FIRST_PROFILE: usize = FIRST_PRESET + 256;

#[derive(Default)]
pub(super) struct MenuState {
    pub profiles: Option<Vec<String>>,
    pub controls: Option<Controls>,
    pub busy: bool,
}

struct Popup {
    handle: HMENU,
    bitmaps: Vec<PresetBitmap>,
}

impl Popup {
    fn new() -> Result<Self> {
        let menu = unsafe { CreatePopupMenu() };
        if menu.is_null() {
            return Err(io::Error::last_os_error()).context("创建托盘菜单");
        }
        Ok(Self {
            handle: menu,
            bitmaps: Vec::new(),
        })
    }

    fn item(&self, flags: MENU_ITEM_FLAGS, id: usize, label: &str) -> Result<()> {
        let label = wide(&label.replace('&', "&&"));
        if unsafe { AppendMenuW(self.handle, flags, id, label.as_ptr()) } == 0 {
            return Err(io::Error::last_os_error()).context("添加托盘菜单项");
        }
        Ok(())
    }

    fn profiles(&mut self, profiles: Option<&[String]>) -> Result<()> {
        let submenu = Self::new()?;
        match profiles {
            Some([]) => submenu.item(MF_GRAYED, 0, "暂无档案")?,
            Some(names) => {
                for (index, name) in names.iter().enumerate() {
                    submenu.item(MF_STRING, FIRST_PROFILE + index, name)?;
                }
            }
            None => submenu.item(MF_GRAYED, 0, "档案读取失败")?,
        }
        self.submenu(submenu, "配置档案")
    }

    fn presets(&mut self, state: &MenuState) -> Result<()> {
        self.item(MF_GRAYED, 0, "预设模式")?;
        let Some(controls) = &state.controls else {
            return self.item(MF_GRAYED, 0, READING_HARDWARE);
        };
        match &controls.presets {
            Ok(presets) => {
                for (mode, label) in &presets.choices {
                    let current = *mode == presets.current;
                    let flags = control_flags(current, state.busy || presets.blocked.is_some());
                    let id = FIRST_PRESET + usize::from(*mode);
                    self.item(flags, id, label)?;
                    if let Err(error) = self.decorate_preset(id, *mode) {
                        log::warn!("[tray] 预设 {mode} 图标：{error:#}");
                    }
                }
                if let Some(reason) = &presets.blocked {
                    self.note(reason, "预设暂不可切换")?;
                }
                Ok(())
            }
            Err(reason) => self.note(reason, "预设状态读取失败"),
        }
    }

    fn decorate_preset(&mut self, id: usize, mode: u8) -> Result<()> {
        let Some((unchecked, checked)) = PresetBitmap::pair(mode)? else {
            return Ok(());
        };
        // 项位图会顶掉对勾，所以图标挂到勾选位图：勾选时显示「对勾｜图标」。
        let info = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_CHECKMARKS,
            hbmpChecked: checked.0,
            hbmpUnchecked: unchecked.0,
            ..Default::default()
        };
        if unsafe { SetMenuItemInfoW(self.handle, id as u32, 0, &info) } == 0 {
            return Err(io::Error::last_os_error()).context("设置预设菜单位图");
        }
        self.bitmaps.extend([checked, unchecked]);
        Ok(())
    }

    fn fans(&mut self, state: &MenuState) -> Result<()> {
        let submenu = Self::new()?;
        match state.controls.as_ref().map(|controls| &controls.fans) {
            Some(Ok(fans)) => {
                submenu.item(MF_GRAYED, 0, &format!("当前 · {}", fans.label))?;
                for action in &fans.choices {
                    let current = Some(*action) == fans.active;
                    let flags =
                        control_flags(current, current || state.busy || fans.blocked.is_some());
                    let (id, label) = match action {
                        FanAction::Auto => (FAN_AUTO, "自动"),
                        FanAction::FullSpeed => (FAN_FULL_SPEED, "全速（需确认）"),
                    };
                    submenu.item(flags, id, label)?;
                }
                if let Some(reason) = &fans.blocked {
                    submenu.note(reason, "风扇模式暂不可切换")?;
                }
            }
            Some(Err(reason)) => submenu.note(reason, "风扇状态读取失败")?,
            None => submenu.item(MF_GRAYED, 0, READING_HARDWARE)?,
        }
        submenu.item(MF_STRING, COOLING, "打开散热控制")?;
        self.submenu(submenu, "风扇散热")
    }

    fn note(&self, reason: &str, summary: &str) -> Result<()> {
        if reason.chars().nth(44).is_some() || reason.contains('…') || reason.contains("...") {
            log::warn!("[tray] {summary}：{reason}");
            return self.item(MF_GRAYED, 0, summary);
        }
        self.item(MF_GRAYED, 0, &reason.replace(['\r', '\n', '\t'], " "))
    }

    fn submenu(&mut self, mut submenu: Self, label: &str) -> Result<()> {
        self.item(MF_POPUP, submenu.handle as usize, label)?;
        submenu.handle = null_mut(); // 由根菜单析构子菜单。
        self.bitmaps.append(&mut submenu.bitmaps);
        Ok(())
    }
}

impl Drop for Popup {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { DestroyMenu(self.handle) };
        }
    }
}

fn control_flags(current: bool, disabled: bool) -> MENU_ITEM_FLAGS {
    let checked = if current { MF_CHECKED } else { MF_UNCHECKED };
    let enabled = if disabled { MF_GRAYED } else { MF_ENABLED };
    MF_STRING | checked | enabled
}

fn build(state: &MenuState, closing: bool) -> Result<Popup> {
    let mut menu = Popup::new()?;
    if closing {
        menu.item(MF_GRAYED, QUIT, "正在退出")?;
    } else {
        menu.item(MF_STRING, SHOW, "显示主窗口")?;
        menu.presets(state)?;
        menu.fans(state)?;
        menu.profiles(state.profiles.as_deref())?;
        menu.item(MF_STRING, QUIT, "退出")?;
    }
    Ok(menu)
}

pub(super) fn popup(
    window: HWND,
    at: POINT,
    state: &MenuState,
    closing: bool,
) -> Result<Option<Action>> {
    let menu = build(state, closing)?;
    let selected = unsafe {
        SetForegroundWindow(window);
        let alignment = if GetSystemMetrics(SM_MENUDROPALIGNMENT) == 0 {
            TPM_LEFTALIGN
        } else {
            TPM_RIGHTALIGN
        };
        let command = TrackPopupMenu(
            menu.handle,
            TPM_NONOTIFY | TPM_RETURNCMD | TPM_RIGHTBUTTON | alignment,
            at.x,
            at.y,
            0,
            window,
            null(),
        );
        PostMessageW(window, WM_NULL, 0, 0);
        command as usize
    };
    Ok(action(selected, state))
}

fn action(selected: usize, state: &MenuState) -> Option<Action> {
    let control = match selected {
        0 => None,
        SHOW => return Some(Action::Show),
        QUIT => return Some(Action::Quit),
        COOLING => return Some(Action::Cooling),
        FAN_AUTO => Some(Control::Fan(FanAction::Auto)),
        FAN_FULL_SPEED => Some(Control::Fan(FanAction::FullSpeed)),
        id if id >= FIRST_PROFILE => {
            return state
                .profiles
                .as_ref()?
                .get(id - FIRST_PROFILE)
                .cloned()
                .map(Action::Profile);
        }
        id if id < FIRST_PRESET + ccc_core::oem_events::POWER_MODE_COUNT as usize => {
            Some(Control::Preset((id - FIRST_PRESET) as u8))
        }
        _ => None,
    }?;
    Some(Action::Control(control))
}

#[cfg(test)]
mod tests {
    use super::MenuState as State;
    use super::*;
    use windows_sys::Win32::Graphics::Gdi::{DIBSECTION, GetObjectW, HBITMAP};

    fn menu_label(menu: HMENU, item: u32, flags: MENU_ITEM_FLAGS) -> String {
        let mut label = [0u16; 128];
        let len =
            unsafe { GetMenuStringW(menu, item, label.as_mut_ptr(), label.len() as i32, flags) };
        String::from_utf16(&label[..len as usize]).unwrap()
    }

    #[test]
    fn tray_shows_the_same_loading_status_in_menu_and_tooltip() {
        let menu = build(&State::default(), false).unwrap();
        let fans = unsafe { GetSubMenu(menu.handle, 3) };
        assert_eq!(
            menu_label(menu.handle, 2, MF_BYPOSITION),
            "正在读取硬件状态"
        );
        assert_eq!(menu_label(fans, 0, MF_BYPOSITION), "正在读取硬件状态");
        assert_eq!(
            super::super::State::default().tooltip(),
            "Colorful Control Center\n正在读取硬件状态"
        );
    }

    #[test]
    fn tooltip_uses_independent_telemetry() {
        let state = super::super::State {
            control_labels: Some(("性能", "全速")),
            temperatures: "\nCPU 42°C".into(),
            busy: false,
        };
        assert_eq!(
            state.tooltip(),
            "Colorful Control Center\n预设 性能 · 风扇 全速\nCPU 42°C"
        );
    }

    #[test]
    fn native_menu_keeps_quit_available_with_empty_or_unreadable_profiles() {
        for profiles in [Some(Vec::new()), None] {
            let view = State {
                profiles,
                ..State::default()
            };
            let menu = build(&view, false).unwrap();
            assert!(matches!(action(QUIT, &view), Some(Action::Quit)));
            let state = unsafe { GetMenuState(menu.handle, QUIT as u32, MF_BYCOMMAND) };
            assert_eq!(state & MF_GRAYED, 0);
        }
    }

    #[test]
    fn native_menu_indicates_exit_in_progress() {
        let menu = build(&State::default(), true).unwrap();
        assert_eq!(unsafe { GetMenuItemCount(menu.handle) }, 1);
        assert_eq!(
            menu_label(menu.handle, QUIT as u32, MF_BYCOMMAND),
            "正在退出"
        );
        assert_ne!(
            unsafe { GetMenuState(menu.handle, QUIT as u32, MF_BYCOMMAND) } & MF_GRAYED,
            0
        );
    }

    #[test]
    fn native_menu_shows_complete_status_in_place_of_long_diagnostics() {
        let mut view = four_preset_view();
        let controls = view.controls.as_mut().unwrap();
        controls.presets.as_mut().unwrap().blocked = Some("原厂软件仍在运行".into());
        controls.fans = Err("风扇状态诊断".repeat(10));
        let menu = build(&view, false).unwrap();
        let fans = unsafe { GetSubMenu(menu.handle, 7) };
        assert_eq!(
            menu_label(menu.handle, 6, MF_BYPOSITION),
            "原厂软件仍在运行"
        );
        assert_eq!(menu_label(fans, 0, MF_BYPOSITION), "风扇状态读取失败");
    }

    fn four_preset_view() -> State {
        use super::super::{Controls, Fans, Presets};
        State {
            controls: Some(Controls {
                presets: Ok(Presets {
                    current: 2,
                    label: "性能",
                    choices: vec![(1, "省电"), (0, "安静"), (3, "娱乐"), (2, "性能")],
                    blocked: None,
                }),
                fans: Ok(Fans {
                    current: 1,
                    label: "全速",
                    active: Some(FanAction::FullSpeed),
                    choices: vec![FanAction::Auto, FanAction::FullSpeed],
                    blocked: None,
                }),
            }),
            profiles: Some(vec!["隔离档案".into()]),
            ..State::default()
        }
    }

    #[test]
    fn native_menu_places_supported_presets_in_root_and_routes_distinct_actions() {
        let view = four_preset_view();
        let menu = build(&view, false).unwrap();
        let item = |id| unsafe { GetMenuState(menu.handle, id as u32, MF_BYCOMMAND) };
        assert_eq!(unsafe { GetMenuItemCount(menu.handle) }, 9);
        for (index, mode) in [1, 0, 3, 2].into_iter().enumerate() {
            assert_eq!(
                unsafe { GetMenuItemID(menu.handle, index as i32 + 2) } as usize,
                FIRST_PRESET + mode
            );
            assert_eq!(
                item(FIRST_PRESET + mode) & MF_CHECKED,
                if mode == 2 { MF_CHECKED } else { 0 }
            );
        }
        assert_eq!(item(FIRST_PRESET + 4), u32::MAX);
        assert!(action(FIRST_PRESET + 4, &view).is_none());
        let fans = unsafe { GetSubMenu(menu.handle, 6) };
        assert_eq!(unsafe { GetMenuItemCount(fans) }, 4);
        assert_eq!(unsafe { GetMenuItemID(fans, 3) } as usize, COOLING);
        assert_eq!(menu_label(fans, FAN_AUTO as u32, MF_BYCOMMAND), "自动");
        assert_eq!(
            menu_label(fans, FAN_FULL_SPEED as u32, MF_BYCOMMAND),
            "全速（需确认）"
        );
        assert_eq!(
            menu_label(fans, COOLING as u32, MF_BYCOMMAND),
            "打开散热控制"
        );
        assert_eq!(item(FAN_FULL_SPEED) & MF_CHECKED, MF_CHECKED);
        assert!(matches!(
            action(FIRST_PRESET + 1, &view),
            Some(Action::Control(Control::Preset(1)))
        ));
        assert!(matches!(
            action(FAN_AUTO, &view),
            Some(Action::Control(Control::Fan(FanAction::Auto)))
        ));
        assert!(matches!(action(COOLING, &view), Some(Action::Cooling)));
        assert!(
            matches!(action(FIRST_PROFILE, &view), Some(Action::Profile(name)) if name == "隔离档案")
        );
        assert!(action(FIRST_PROFILE + 1, &view).is_none());
        let busy = build(&State { busy: true, ..view }, false).unwrap();
        assert_ne!(
            unsafe { GetMenuState(busy.handle, (FIRST_PRESET + 1) as u32, MF_BYCOMMAND) }
                & MF_GRAYED,
            0
        );
        assert_eq!(
            unsafe { GetMenuState(busy.handle, COOLING as u32, MF_BYCOMMAND) } & MF_GRAYED,
            0
        );
    }

    fn preset_item(menu: &Popup, mode: usize) -> MENUITEMINFOW {
        let mut item = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_BITMAP | MIIM_CHECKMARKS,
            ..Default::default()
        };
        assert_ne!(
            unsafe { GetMenuItemInfoW(menu.handle, (FIRST_PRESET + mode) as u32, 0, &mut item) },
            0
        );
        item
    }

    /// 读取勾选位图的像素，并逐行切成（对勾格，图标格）。
    fn cells(bitmap: HBITMAP) -> (Vec<u8>, Vec<u8>) {
        let mut info = DIBSECTION::default();
        let size = size_of::<DIBSECTION>() as i32;
        assert_eq!(
            unsafe { GetObjectW(bitmap, size, (&mut info as *mut DIBSECTION).cast()) },
            size
        );
        assert_eq!(info.dsBm.bmBitsPixel, 32);
        assert_eq!(info.dsBm.bmWidth, 2 * info.dsBm.bmHeight);
        assert!(!info.dsBm.bmBits.is_null());
        let row = info.dsBm.bmWidthBytes as usize;
        let len = row * info.dsBm.bmHeight as usize;
        let mut cells = (Vec::new(), Vec::new());
        for line in unsafe { std::slice::from_raw_parts(info.dsBm.bmBits.cast::<u8>(), len) }
            .chunks_exact(row)
        {
            cells.0.extend_from_slice(&line[..row / 2]);
            cells.1.extend_from_slice(&line[row / 2..]);
        }
        cells
    }

    #[test]
    fn native_menu_keeps_checkmark_and_attaches_transparent_preset_artwork() {
        let menu = build(&four_preset_view(), false).unwrap();
        let images = (0..=3)
            .map(|mode| {
                let item = preset_item(&menu, mode);
                assert!(item.hbmpItem.is_null());
                assert!(!item.hbmpChecked.is_null() && !item.hbmpUnchecked.is_null());
                (cells(item.hbmpUnchecked), cells(item.hbmpChecked))
            })
            .collect::<Vec<_>>();
        for (unchecked, checked) in &images {
            // 未勾选时对勾格留白；勾选后画上对勾，图标格保持不变。
            assert!(unchecked.0.chunks_exact(4).all(|pixel| pixel[3] == 0));
            assert!(checked.0.chunks_exact(4).any(|pixel| pixel[3] > 0));
            assert_eq!(checked.1, unchecked.1);
            let artwork = &unchecked.1;
            assert!(artwork.chunks_exact(4).any(|pixel| pixel[3] == 0));
            assert!(artwork.chunks_exact(4).any(|pixel| pixel[3] > 0));
        }
        assert!(images.windows(2).all(|pair| pair[0].1.0 == pair[1].1.0));
        assert_eq!(
            images
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            4
        );
        let state = unsafe { GetMenuState(menu.handle, (FIRST_PRESET + 2) as u32, MF_BYCOMMAND) };
        assert_eq!(state & (MF_CHECKED | MF_GRAYED), MF_CHECKED);
    }

    #[test]
    fn native_menu_keeps_the_system_checkmark_for_an_unknown_preset_icon() {
        let mut menu = Popup::new().unwrap();
        let mode = 99;
        menu.item(MF_STRING | MF_CHECKED, FIRST_PRESET + mode, "未知")
            .unwrap();
        menu.decorate_preset(FIRST_PRESET + mode, mode as u8)
            .unwrap();
        let item = preset_item(&menu, mode);
        assert!(
            item.hbmpItem.is_null() && item.hbmpChecked.is_null() && item.hbmpUnchecked.is_null()
        );
    }
}
