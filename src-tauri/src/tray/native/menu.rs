use super::{Action, Control, FanAction, State, wide};
use anyhow::{Context, Result};
use std::{io, ptr::null};
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

struct Popup(HMENU);

impl Popup {
    fn new() -> Result<Self> {
        let menu = unsafe { CreatePopupMenu() };
        if menu.is_null() {
            return Err(io::Error::last_os_error()).context("创建托盘菜单");
        }
        Ok(Self(menu))
    }

    fn item(&self, flags: MENU_ITEM_FLAGS, id: usize, label: &str) -> Result<()> {
        let label = wide(&label.replace('&', "&&"));
        if unsafe { AppendMenuW(self.0, flags, id, label.as_ptr()) } == 0 {
            return Err(io::Error::last_os_error()).context("添加托盘菜单项");
        }
        Ok(())
    }

    fn profiles(&self, profiles: Option<&[String]>) -> Result<()> {
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

    fn presets(&self, state: &State) -> Result<()> {
        self.item(MF_GRAYED, 0, "预设模式")?;
        let Some(controls) = &state.controls else {
            return self.item(MF_GRAYED, 0, "正在读取硬件状态…");
        };
        match &controls.presets {
            Ok(presets) => {
                for (mode, label) in &presets.choices {
                    let current = *mode == presets.current;
                    let flags = control_flags(current, state.busy || presets.blocked.is_some());
                    self.item(flags, FIRST_PRESET + usize::from(*mode), label)?;
                }
                if let Some(reason) = &presets.blocked {
                    self.note(reason)?;
                }
                Ok(())
            }
            Err(reason) => self.note(reason),
        }
    }

    fn fans(&self, state: &State) -> Result<()> {
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

    fn submenu(&self, submenu: Self, label: &str) -> Result<()> {
        self.item(MF_POPUP, submenu.0 as usize, label)?;
        std::mem::forget(submenu); // 由根菜单析构子菜单。
        Ok(())
    }
}

impl Drop for Popup {
    fn drop(&mut self) {
        unsafe { DestroyMenu(self.0) };
    }
}

fn control_flags(current: bool, disabled: bool) -> MENU_ITEM_FLAGS {
    let checked = if current { MF_CHECKED } else { MF_UNCHECKED };
    let enabled = if disabled { MF_GRAYED } else { MF_ENABLED };
    MF_STRING | checked | enabled
}

fn build(state: &State, closing: bool) -> Result<Popup> {
    let menu = Popup::new()?;
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
            menu.0,
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

    #[test]
    fn native_menu_keeps_quit_available_with_empty_or_unreadable_profiles() {
        for profiles in [Some(std::sync::Arc::from([])), None] {
            let view = State {
                profiles,
                ..State::default()
            };
            let menu = build(&view, false).unwrap();
            assert!(matches!(action(QUIT, &view), Some(Action::Quit)));
            let state = unsafe { GetMenuState(menu.0, QUIT as u32, MF_BYCOMMAND) };
            assert_eq!(state & MF_GRAYED, 0);
        }
    }

    #[test]
    fn native_menu_indicates_exit_in_progress() {
        let menu = build(&State::default(), true).unwrap();
        assert_eq!(unsafe { GetMenuItemCount(menu.0) }, 1);
        assert_ne!(
            unsafe { GetMenuState(menu.0, QUIT as u32, MF_BYCOMMAND) } & MF_GRAYED,
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
        let item = |id| unsafe { GetMenuState(menu.0, id as u32, MF_BYCOMMAND) };
        assert_eq!(unsafe { GetMenuItemCount(menu.0) }, 7);
        assert_eq!(
            unsafe { GetMenuItemID(menu.0, 2) } as usize,
            FIRST_PRESET + 1
        );
        let fans = unsafe { GetSubMenu(menu.0, 4) };
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
            unsafe { GetMenuState(busy.0, (FIRST_PRESET + 1) as u32, MF_BYCOMMAND) } & MF_GRAYED,
            0
        );
        assert_eq!(
            unsafe { GetMenuState(busy.0, COOLING as u32, MF_BYCOMMAND) } & MF_GRAYED,
            0
        );
    }
}
