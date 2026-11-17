use anyhow::{Context, Result, anyhow, bail};
use ccc_core::oem_events::Action;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    PowerMode,
    Fan,
    KeyboardBrightness,
    KeyboardSwitch,
    KeyboardZones,
    KeyboardColor,
    MicrophoneMute,
    Touchpad,
    FnLock,
    Mappings,
    OpenConsole,
    StateNotifications,
    KeyboardBrightnessEvent,
    CameraReports,
    ScreenBrightness,
    PanelHotkey,
    GpuTurbo,
    BatteryThresholds,
    PowerDeliveryEvent,
    BatteryColdWarning,
    BatteryLowBrightness,
    BatteryLowAction,
    OpenKeyboard,
    DisplayRotation,
    BatteryAlert,
    SystemRecovery,
    SystemShutdown,
    LidAction,
    AirplaneMode,
    PanelEvent,
}
pub const SCOPES: [Scope; 30] = [
    Scope::PowerMode,
    Scope::Fan,
    Scope::KeyboardBrightness,
    Scope::KeyboardSwitch,
    Scope::KeyboardZones,
    Scope::KeyboardColor,
    Scope::MicrophoneMute,
    Scope::Touchpad,
    Scope::FnLock,
    Scope::Mappings,
    Scope::OpenConsole,
    Scope::StateNotifications,
    Scope::KeyboardBrightnessEvent,
    Scope::CameraReports,
    Scope::ScreenBrightness,
    Scope::PanelHotkey,
    Scope::GpuTurbo,
    Scope::BatteryThresholds,
    Scope::PowerDeliveryEvent,
    Scope::BatteryColdWarning,
    Scope::BatteryLowBrightness,
    Scope::BatteryLowAction,
    Scope::OpenKeyboard,
    Scope::DisplayRotation,
    Scope::BatteryAlert,
    Scope::SystemRecovery,
    Scope::SystemShutdown,
    Scope::LidAction,
    Scope::AirplaneMode,
    Scope::PanelEvent,
];
impl Scope {
    pub fn for_action(action: Action) -> Self {
        match action {
            Action::CycleMode => Self::PowerMode,
            Action::FanMode(_) => Self::Fan,
            Action::KeyboardBrightness(_) => Self::KeyboardBrightness,
            Action::KeyboardBrightnessEvent => Self::KeyboardBrightnessEvent,
            Action::ToggleKeyboard { .. } => Self::KeyboardSwitch,
            Action::ToggleKeyboardZone(_) => Self::KeyboardZones,
            Action::CycleKeyboardColor => Self::KeyboardColor,
            Action::ToggleMicrophones => Self::MicrophoneMute,
            Action::ToggleTouchpad => Self::Touchpad,
            Action::ToggleFnLock => Self::FnLock,
            Action::ToggleMappings => Self::Mappings,
            Action::OpenControlCenter => Self::OpenConsole,
            Action::Notify(_) => Self::StateNotifications,
            Action::CameraReport(_) => Self::CameraReports,
            Action::DisplayBrightness(_) => Self::ScreenBrightness,
            Action::PanelHotkey => Self::PanelHotkey,
            Action::GpuTurbo(_) => Self::GpuTurbo,
            Action::BatteryThreshold(_) => Self::BatteryThresholds,
            Action::PowerDeliveryEvent => Self::PowerDeliveryEvent,
            Action::BatteryColdWarning => Self::BatteryColdWarning,
            Action::BatteryLowBrightness(_) => Self::BatteryLowBrightness,
            Action::BatteryLowAction(_) => Self::BatteryLowAction,
            Action::OpenKeyboard => Self::OpenKeyboard,
            Action::DisplayRotation => Self::DisplayRotation,
            Action::BatteryAlert { .. } => Self::BatteryAlert,
            Action::OpenRecovery => Self::SystemRecovery,
            Action::SystemShutdown => Self::SystemShutdown,
            Action::LidAction => Self::LidAction,
            Action::ToggleAirplane => Self::AirplaneMode,
            Action::PanelEvent => Self::PanelEvent,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub actions: Vec<Scope>,
    pub osd: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            actions: vec![],
            osd: true,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        let unique: BTreeSet<_> = self.actions.iter().copied().collect();
        if unique.len() != self.actions.len()
            || unique.into_iter().collect::<Vec<_>>() != self.actions
        {
            bail!("热键范围重复或顺序无效");
        }
        if self.enabled && self.actions.is_empty() {
            bail!("请至少选择一项热键范围");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub scope: Scope,
    pub identity: Option<String>,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    pub mode_cycle: Option<crate::powerctl::mode::cycle::Profile>,
    pub panel_hotkey: Option<super::panel_hotkey::Profile>,
    pub screen_brightness: Option<super::screen_brightness::Profile>,
    pub gpu_turbo: Option<crate::gpuctl::turbo::Profile>,
    pub battery_thresholds: Option<super::battery_thresholds::Profile>,
    pub pd_event: Option<crate::platform::pd_event::Profile>,
    pub battery_low_brightness: Option<super::screen_brightness::LowBatteryProfile>,
    pub battery_low_action: Option<super::low_battery_action::Profile>,
    pub lid_action: Option<super::system_action::Profile>,
    pub battery_alert: Option<super::battery_alert::Profile>,
    pub capabilities: Vec<Capability>,
    pub blocked_reason: Option<String>,
}
pub fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
pub fn authorization(settings: &Settings, facts: &Facts) -> Result<Option<Vec<(Scope, String)>>> {
    settings.validate()?;
    if !settings.enabled {
        return Ok(None);
    }
    let mut selected = Vec::new();
    for scope in &settings.actions {
        let cap = facts
            .capabilities
            .iter()
            .find(|c| c.scope == *scope)
            .ok_or_else(|| anyhow!("热键能力缺失"))?;
        let identity = cap
            .identity
            .as_ref()
            .ok_or_else(|| anyhow!("{}", cap.reason.as_deref().unwrap_or("热键能力不可用")))?;
        selected.push((*scope, identity.clone()));
    }
    Ok(Some(selected))
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub version: u8,
    pub settings: Settings,
    #[serde(deserialize_with = "Option::deserialize")]
    pub authorization: Option<Vec<(Scope, String)>>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub mode_driver: Option<String>,
}
impl Default for Record {
    fn default() -> Self {
        Self {
            version: 2,
            settings: Settings::default(),
            authorization: None,
            mode_driver: None,
        }
    }
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        if self.version != 2
            || self.settings.enabled != self.authorization.is_some()
            || self.authorization.as_ref().is_some_and(|grants| {
                grants.iter().map(|(scope, _)| *scope).collect::<Vec<_>>() != self.settings.actions
                    || grants.iter().any(|(_, identity)| {
                        identity.is_empty() || identity.len() > 4096 || identity.contains('\0')
                    })
            })
        {
            bail!("热键策略版本或授权记录无效");
        }
        if self.mode_driver.as_ref().is_some_and(|identity| {
            !self.settings.enabled
                || !self.settings.actions.contains(&Scope::PowerMode)
                || identity.len() != 64
                || !identity
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            bail!("电源模式热键的GPU驱动授权无效");
        }
        Ok(())
    }
    pub fn authorize(settings: Settings, facts: &Facts) -> Result<Self> {
        Ok(Self {
            version: 2,
            mode_driver: (settings.enabled && settings.actions.contains(&Scope::PowerMode))
                .then(|| {
                    facts
                        .mode_cycle
                        .as_ref()
                        .and_then(|profile| profile.driver_identity())
                        .map(str::to_owned)
                })
                .flatten(),
            authorization: authorization(&settings, facts)?,
            settings,
        })
    }
    pub fn verify(&self, facts: &Facts) -> Result<()> {
        self.validate()?;
        if let Some(reason) = &facts.blocked_reason {
            bail!(reason.clone());
        }
        if !self.settings.enabled || self.authorization != authorization(&self.settings, facts)? {
            bail!("热键授权范围或设备能力已变化，请重新预览确认");
        }
        if self.settings.actions.contains(&Scope::PowerMode) {
            if let Some(profile) = &facts.mode_cycle {
                profile.check_driver(self.mode_driver.as_deref())?;
            }
        }
        Ok(())
    }
    pub fn verify_action(&self, facts: &Facts, scope: Scope) -> Result<()> {
        self.validate()?;
        if let Some(reason) = &facts.blocked_reason {
            bail!("{reason}");
        }
        if !self.settings.enabled || !self.settings.actions.contains(&scope) {
            bail!("热键范围未经授权");
        }
        let selected = Settings {
            actions: vec![scope],
            ..self.settings.clone()
        };
        let current = authorization(&selected, facts)?.context("热键范围未经授权")?;
        let saved = self
            .authorization
            .as_ref()
            .and_then(|grants| grants.iter().find(|(key, _)| *key == scope));
        if saved != current.first() {
            bail!("热键目标能力变化，请重新确认接管范围");
        }
        if scope == Scope::PowerMode {
            if let Some(profile) = &facts.mode_cycle {
                profile.check_driver(self.mode_driver.as_deref())?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub running: bool,
    pub error: Option<String>,
    pub last_result: Option<String>,
    pub cold_warning: Option<super::battery_cold::Warning>,
    pub navigation_request: Option<NavigationRequest>,
    #[serde(skip)]
    pub warning_sequence: u64,
    #[serde(skip)]
    pub navigation_sequence: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NavigationKind {
    Keyboard,
    DisplayRotation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationRequest {
    pub id: u64,
    pub ready: bool,
    pub kind: NavigationKind,
}
impl Status {
    pub fn stop_clean(&mut self) {
        self.cold_warning = None;
        if self
            .navigation_request
            .as_ref()
            .is_some_and(|request| !request.ready)
        {
            self.navigation_request = None;
        }
    }
    pub fn queue_navigation(&mut self, kind: NavigationKind) -> Result<u64> {
        let next = self
            .navigation_sequence
            .checked_add(1)
            .filter(|value| *value <= 9_007_199_254_740_991)
            .ok_or_else(|| anyhow!("热键导航序号耗尽"))?;
        self.navigation_sequence = next;
        self.navigation_request = Some(NavigationRequest {
            id: next,
            ready: false,
            kind,
        });
        Ok(next)
    }
    pub fn navigation_pending(&self, id: u64) -> bool {
        self.running
            && self
                .navigation_request
                .as_ref()
                .is_some_and(|request| request.id == id && !request.ready)
    }
    pub fn navigation_ready(&mut self, id: u64) -> bool {
        if !self.navigation_pending(id) {
            return false;
        }
        self.navigation_request.as_mut().unwrap().ready = true;
        true
    }
    pub fn acknowledge_navigation(&mut self, id: u64) -> Result<()> {
        if !self
            .navigation_request
            .as_ref()
            .is_some_and(|request| request.id == id && request.ready)
        {
            bail!("热键导航请求已变化，请重新查看");
        }
        self.navigation_request = None;
        Ok(())
    }
    pub fn dismiss_cold_warning(&mut self, expected_id: u64) -> Result<()> {
        if self.cold_warning.as_ref().map(|warning| warning.id) != Some(expected_id) {
            bail!("低温提醒已变化，请重新查看");
        }
        self.cold_warning = None;
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub settings: Settings,
}
