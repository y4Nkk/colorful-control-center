//! FnKey.Arg001_GetEvent的包与动作事实；不消费UI状态广播或执行系统操作。
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet {
    pub code: u8,
    pub data: i32,
}
pub fn decode(message: &str) -> Result<Option<Packet>, String> {
    let Some(value) = message.strip_prefix("Arg001^") else {
        return Ok(None);
    };
    let raw: i32 = value.parse().map_err(|_| "原厂Arg001事件不是32位整数")?;
    if raw.to_string() != value {
        return Err("原厂Arg001事件格式无效".into());
    }
    Ok(Some(Packet {
        code: raw as u8,
        data: raw >> 8,
    }))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    NumLock(bool),
    CapsLock(bool),
    ScrollLock(bool),
    Docking(bool),
    Gps(bool),
    SilentMode(bool),
    MobileNetwork(bool),
    DustRunning,
    TurboFan,
    Charger(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraReport {
    Camera(bool),
    CameraMicrophone(bool),
}
impl CameraReport {
    pub fn saved_camera(self) -> Option<u8> {
        match self {
            Self::Camera(enabled) => Some(u8::from(enabled)),
            Self::CameraMicrophone(true) => Some(1),
            Self::CameraMicrophone(false) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    CycleMode,
    KeyboardBrightness(i8),
    KeyboardBrightnessEvent,
    ToggleKeyboard {
        rgb_only: bool,
    },
    ToggleMicrophones,
    ToggleTouchpad,
    ToggleKeyboardZone(u8),
    CycleKeyboardColor,
    ToggleFnLock,
    ToggleMappings,
    FanMode(u8),
    OpenControlCenter,
    Notify(Notice),
    CameraReport(CameraReport),
    DisplayBrightness(u8),
    PanelHotkey,
    GpuTurbo(u8),
    BatteryThreshold(u8),
    PowerDeliveryEvent,
    BatteryColdWarning,
    BatteryLowBrightness(u8),
    BatteryAlert {
        active: bool,
        brightness: Option<u8>,
    },
    BatteryLowAction(u8),
    OpenKeyboard,
    DisplayRotation,
    OpenRecovery,
    SystemShutdown,
    LidAction,
    ToggleAirplane,
    PanelEvent,
}
impl Action {
    pub fn is_status_feedback(self) -> bool {
        matches!(
            self,
            Self::Notify(_) | Self::CameraReport(_) | Self::BatteryColdWarning
        )
    }
}
impl Packet {
    pub fn action(self) -> Option<Action> {
        Some(match self.code {
            133 => Action::ToggleAirplane,
            84 => Action::PanelEvent,
            89 => Action::CycleMode,
            32 | 129 => Action::KeyboardBrightness(-1),
            33 | 130 => Action::KeyboardBrightness(1),
            138 => Action::KeyboardBrightnessEvent,
            63 => Action::ToggleKeyboard { rgb_only: false },
            158 | 159 => Action::ToggleKeyboard { rgb_only: true },
            38 => Action::ToggleMicrophones,
            252 | 253 => Action::ToggleTouchpad,
            139..=141 => Action::ToggleKeyboardZone(self.code - 139),
            57 => Action::CycleKeyboardColor,
            37 => Action::ToggleFnLock,
            123 => Action::ToggleMappings,
            106 => Action::FanMode(5),
            107 => Action::FanMode(6),
            112 => Action::FanMode(0),
            143 => Action::FanMode(1),
            149 => Action::OpenControlCenter,
            166 | 167 => Action::Notify(Notice::Docking(self.code == 166)),
            184 | 185 => Action::Notify(Notice::Gps(self.code == 185)),
            199 | 200 => Action::Notify(Notice::NumLock(self.code == 200)),
            201 | 202 => Action::Notify(Notice::CapsLock(self.code == 202)),
            203 | 204 => Action::Notify(Notice::ScrollLock(self.code == 204)),
            222 | 223 => Action::Notify(Notice::SilentMode(self.code == 222)),
            234 | 235 => Action::Notify(Notice::MobileNetwork(self.code == 235)),
            64 => Action::Notify(Notice::DustRunning),
            58 => Action::Notify(Notice::TurboFan),
            240 | 241 => Action::Notify(Notice::Charger(self.code == 240)),
            246 | 247 => Action::CameraReport(CameraReport::Camera(self.code == 247)),
            248 | 249 => Action::CameraReport(CameraReport::CameraMicrophone(self.code == 249)),
            78 | 79 | 48..=56 => Action::DisplayBrightness(self.code),
            215 => Action::PanelHotkey,
            96 | 97 => Action::GpuTurbo(self.code),
            209 | 210 => Action::BatteryThreshold(self.code),
            46 => Action::PowerDeliveryEvent,
            47 => Action::BatteryColdWarning,
            59 | 60 => Action::BatteryLowBrightness(self.code),
            186 => Action::BatteryAlert {
                active: true,
                brightness: (self.data & 0x80 != 0).then_some((self.data & 0x7f) as u8),
            },
            187 => Action::BatteryAlert {
                active: false,
                brightness: None,
            },
            44 | 45 => Action::BatteryLowAction(self.code),
            131 => Action::OpenKeyboard,
            124 => Action::DisplayRotation,
            132 => Action::OpenRecovery,
            72 => Action::SystemShutdown,
            192 => Action::LidAction,
            _ => return None,
        })
    }
}
/// FnKey的158/159分支只适用于列出的原始型号；63不作此限制。
pub fn keyboard_toggle_type_allowed(rgb_only: bool, raw_type: u8) -> bool {
    !rgb_only || matches!(raw_type, 2 | 3 | 6 | 22 | 23 | 38)
}
pub fn keyboard_navigation_type_allowed(raw_type: u8) -> bool {
    matches!(raw_type, 1 | 2 | 3 | 6 | 22 | 23 | 38)
}
pub const POWER_MODE_COUNT: u8 = 4;

pub const fn is_power_mode(mode: u8) -> bool {
    mode < POWER_MODE_COUNT
}

/// FnKey 循环顺序为安静、性能、娱乐、省电，跳过固件未声明项。
pub fn next_mode(current: u8, available: &[u8]) -> Result<u8, String> {
    if !is_power_mode(current) {
        return Err("当前原厂模式未知".into());
    }
    if available.iter().any(|mode| !is_power_mode(*mode)) {
        return Err("未知原厂模式候选".into());
    }
    let order: Vec<_> = [0, 2, 3, 1]
        .into_iter()
        .filter(|mode| available.contains(mode))
        .collect();
    if order.is_empty() {
        return Err("没有原厂模式候选".into());
    }
    let index = order
        .iter()
        .position(|mode| *mode == current)
        .map_or(0, |index| (index + 1) % order.len());
    Ok(order[index])
}
#[derive(Default)]
pub struct Gate {
    last_mode: Option<Duration>,
}
impl Gate {
    pub fn accept(&mut self, action: Action, at: Duration) -> bool {
        if action != Action::CycleMode {
            return true;
        }
        if self.last_mode.is_some_and(|last| {
            at.checked_sub(last)
                .is_none_or(|elapsed| elapsed < Duration::from_secs(3))
        }) {
            return false;
        }
        self.last_mode = Some(at);
        true
    }
}

#[cfg(test)]
#[path = "oem_events_tests.rs"]
mod tests;
