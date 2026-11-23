use super::preset_bitmap::PresetBitmap;
use super::{Action, Control, FanAction, State, wide};
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
const FAN_FIRMWARE: usize = 4;
const FAN_FULL_SPEED: usize = 5;
const FIRST_PRESET: usize = 6;
const FIRST_PROFILE: usize = FIRST_PRESET + 256;

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

    fn presets(&mut self, state: &State) -> Result<()> {
        self.item(MF_GRAYED, 0, "预设模式")?;
        let Some(controls) = &state.controls else {
            return self.item(MF_GRAYED, 0, "正在读取硬件状态…");
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
                    self.note(reason)?;
                }
                Ok(())
            }
            Err(reason) => self.note(reason),
        }
    }

    fn decorate_preset(&mut self, id: usize, mode: u8) -> Result<()> {
        let Some(bitmap) = PresetBitmap::load(mode)? else {
            return Ok(());
        };
        let info = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_BITMAP,
            hbmpItem: bitmap.0,
            ..Default::default()
        };
        if unsafe { SetMenuItemInfoW(self.handle, id as u32, 0, &info) } == 0 {
            return Err(io::Error::last_os_error()).context("设置预设菜单位图");
        }
        self.bitmaps.push(bitmap);
        Ok(())
    }

    fn fans(&mut self, state: &State) -> Result<()> {
        let submenu = Self::new()?;
        match state.controls.as_ref().map(|controls| &controls.fans) {
            Some(Ok(fans)) => {
                submenu.item(MF_GRAYED, 0, &format!("当前 · {}", fans.label))?;
                for action in &fans.choices {
                    let current = Some(*action) == fans.active;
                    let flags =
                        control_flags(current, current || state.busy || fans.blocked.is_some());
                    let (id, label) = match action {
                        FanAction::Firmware => (FAN_FIRMWARE, "交还固件控制"),
                        FanAction::FullSpeed => (FAN_FULL_SPEED, "全速…"),
                    };
                    submenu.item(flags, id, label)?;
                }
                if let Some(reason) = &fans.blocked {
                    submenu.note(reason)?;
                }
            }
            Some(Err(reason)) => submenu.note(reason)?,
            None => submenu.item(MF_GRAYED, 0, "正在读取硬件状态…")?,
        }
        submenu.item(MF_STRING, COOLING, "风扇设置…")?;
        self.submenu(submenu, "风扇散热")
    }

    fn note(&self, reason: &str) -> Result<()> {
        let mut chars = reason.chars();
        let mut label = chars.by_ref().take(44).collect::<String>();
        if chars.next().is_some() {
            label.push('…');
        }
        self.item(MF_GRAYED, 0, &label.replace(['\r', '\n', '\t'], " "))
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

fn build(state: &State, closing: bool) -> Result<Popup> {
    let mut menu = Popup::new()?;
    if closing {
        menu.item(MF_GRAYED, QUIT, "正在退出…")?;
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
    state: &State,
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

fn action(selected: usize, state: &State) -> Option<Action> {
    let control = match selected {
        0 => None,
        SHOW => return Some(Action::Show),
        QUIT => return Some(Action::Quit),
        COOLING => return Some(Action::Cooling),
        FAN_FIRMWARE => Some(Control::Fan(FanAction::Firmware)),
        FAN_FULL_SPEED => Some(Control::Fan(FanAction::FullSpeed)),
        id if id >= FIRST_PROFILE => {
            return state
                .profiles
                .as_ref()?
                .get(id - FIRST_PROFILE)
                .cloned()
                .map(Action::Profile);
        }
        id => Some(Control::Preset((id - FIRST_PRESET) as u8)),
    }?;
    Some(Action::Control(control))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Graphics::Gdi::{DIBSECTION, GetObjectW};

    #[test]
    fn native_menu_keeps_quit_available_with_empty_or_unreadable_profiles() {
        for profiles in [Some(std::sync::Arc::from([])), None] {
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
        assert_ne!(
            unsafe { GetMenuState(menu.handle, QUIT as u32, MF_BYCOMMAND) } & MF_GRAYED,
            0
        );
    }

    #[test]
    fn native_menu_places_supported_presets_in_root_and_routes_distinct_actions() {
        use super::super::{Controls, Fans, Presets};
        let view = State {
            controls: Some(Controls {
                presets: Ok(Presets {
                    current: 4,
                    label: "静音增强",
                    choices: vec![(1, "省电"), (4, "静音增强")],
                    blocked: None,
                }),
                fans: Ok(Fans {
                    current: 1,
                    label: "全速",
                    active: Some(FanAction::FullSpeed),
                    choices: vec![FanAction::Firmware, FanAction::FullSpeed],
                    blocked: None,
                }),
            }),
            profiles: Some(std::sync::Arc::from(["隔离档案".into()])),
            temperatures: "\nCPU 42°C".into(),
            ..State::default()
        };
        let menu = build(&view, false).unwrap();
        let item = |id| unsafe { GetMenuState(menu.handle, id as u32, MF_BYCOMMAND) };
        assert_eq!(unsafe { GetMenuItemCount(menu.handle) }, 7);
        assert_eq!(
            unsafe { GetMenuItemID(menu.handle, 2) } as usize,
            FIRST_PRESET + 1
        );
        let fans = unsafe { GetSubMenu(menu.handle, 4) };
        assert_eq!(unsafe { GetMenuItemCount(fans) }, 4);
        assert_eq!(unsafe { GetMenuItemID(fans, 3) } as usize, COOLING);
        assert_eq!(item(FIRST_PRESET), u32::MAX);
        assert_eq!(
            item(FIRST_PRESET + 4) & (MF_CHECKED | MF_GRAYED),
            MF_CHECKED
        );
        assert_eq!(item(FAN_FULL_SPEED) & MF_CHECKED, MF_CHECKED);
        assert!(matches!(
            action(FIRST_PRESET + 1, &view),
            Some(Action::Control(Control::Preset(1)))
        ));
        assert!(matches!(
            action(FAN_FIRMWARE, &view),
            Some(Action::Control(Control::Fan(FanAction::Firmware)))
        ));
        assert!(matches!(action(COOLING, &view), Some(Action::Cooling)));
        assert!(
            matches!(action(FIRST_PROFILE, &view), Some(Action::Profile(name)) if name == "隔离档案")
        );
        assert!(action(FIRST_PROFILE + 1, &view).is_none());
        assert_eq!(
            view.tooltip(),
            "Colorful Control Center\n预设 静音增强 · 风扇 全速\nCPU 42°C"
        );
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

    fn preset_pixels(menu: &Popup, mode: usize) -> Option<Vec<u8>> {
        let mut item = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_BITMAP,
            ..Default::default()
        };
        assert_ne!(
            unsafe { GetMenuItemInfoW(menu.handle, (FIRST_PRESET + mode) as u32, 0, &mut item) },
            0
        );
        if item.hbmpItem.is_null() {
            return None;
        }
        let mut bitmap = DIBSECTION::default();
        let size = size_of::<DIBSECTION>() as i32;
        assert_eq!(
            unsafe { GetObjectW(item.hbmpItem, size, (&mut bitmap as *mut DIBSECTION).cast()) },
            size
        );
        let info = bitmap.dsBm;
        assert_eq!(info.bmBitsPixel, 32);
        assert!(!info.bmBits.is_null());
        let len = (info.bmWidthBytes * info.bmHeight) as usize;
        Some(unsafe { std::slice::from_raw_parts(info.bmBits.cast(), len) }.to_vec())
    }

    #[test]
    fn native_menu_keeps_checkmark_and_attaches_transparent_preset_artwork() {
        use super::super::{Controls, Presets};
        let view = State {
            controls: Some(Controls {
                presets: Ok(Presets {
                    current: 2,
                    label: "性能",
                    choices: vec![
                        (1, "省电"),
                        (0, "安静"),
                        (3, "娱乐"),
                        (2, "性能"),
                        (4, "静音增强"),
                        (99, "未知"),
                    ],
                    blocked: None,
                }),
                fans: Err("未读取风扇状态".into()),
            }),
            ..State::default()
        };
        let menu = build(&view, false).unwrap();
        let images = (0..=4)
            .map(|mode| preset_pixels(&menu, mode).unwrap())
            .collect::<Vec<_>>();
        for image in &images {
            assert!(image.chunks_exact(4).any(|pixel| pixel[3] == 0));
            assert!(image.chunks_exact(4).any(|pixel| pixel[3] > 0));
        }
        assert_eq!(images[0], images[4]);
        assert_eq!(
            images
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            4
        );
        assert!(preset_pixels(&menu, 99).is_none());
        let state = unsafe { GetMenuState(menu.handle, (FIRST_PRESET + 2) as u32, MF_BYCOMMAND) };
        assert_eq!(state & (MF_CHECKED | MF_GRAYED), MF_CHECKED);
    }
}
