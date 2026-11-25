use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

macro_rules! preference_enum {
    ($name:ident { $($variant:ident),+ }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub enum $name { $($variant),+ }
    };
}
preference_enum!(Theme {
    System,
    Light,
    Dark
});
preference_enum!(Motion { System, Reduce });
preference_enum!(CloseBehavior { HideToTray, Exit });
preference_enum!(StartupPage {
    Dashboard,
    LastVisited
});
preference_enum!(LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Silent
});

impl LogLevel {
    pub fn filter(self) -> log::LevelFilter {
        match self {
            Self::Debug => log::LevelFilter::Debug,
            Self::Info => log::LevelFilter::Info,
            Self::Warn => log::LevelFilter::Warn,
            Self::Error => log::LevelFilter::Error,
            Self::Silent => log::LevelFilter::Off,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub theme: Theme,
    pub interface_font: String,
    pub interface_scale: u16,
    pub motion: Motion,
    pub close_behavior: CloseBehavior,
    pub always_on_top: bool,
    pub start_minimized: bool,
    #[serde(rename = "closeOemOnStartup", default = "default_auto_close_oem")]
    pub auto_close_oem: bool,
    pub startup_page: StartupPage,
    pub global_shortcut: String,
    pub runtime_log_level: LogLevel,
    pub hardware_acceleration: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub theme: Theme,
    pub interface_font: String,
    pub interface_scale: u16,
    pub motion: Motion,
}
impl From<&Preferences> for Appearance {
    fn from(value: &Preferences) -> Self {
        Self {
            theme: value.theme,
            interface_font: value.interface_font.clone(),
            interface_scale: value.interface_scale,
            motion: value.motion,
        }
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            interface_font: "system".into(),
            interface_scale: 100,
            motion: Motion::System,
            close_behavior: CloseBehavior::HideToTray,
            always_on_top: false,
            start_minimized: false,
            auto_close_oem: default_auto_close_oem(),
            startup_page: StartupPage::Dashboard,
            global_shortcut: String::new(),
            runtime_log_level: LogLevel::Info,
            hardware_acceleration: true,
        }
    }
}

fn default_auto_close_oem() -> bool {
    true
}

impl Preferences {
    pub fn validate(&self) -> Result<()> {
        if ![90, 100, 110, 125].contains(&self.interface_scale) {
            bail!("界面缩放必须为 90%、100%、110% 或 125%");
        }
        let font = &self.interface_font;
        if font.is_empty()
            || font.len() > 256
            || font.trim() != font
            || font
                .chars()
                .any(|c| c.is_control() || matches!(c, '"' | '\'' | '\\' | ';'))
        {
            bail!("界面字体名称无效");
        }
        super::shortcut::parse(&self.global_shortcut)?;
        Ok(())
    }
}
