//! FlexiKey数据及HKFltrLib执行计划；扫描码与虚拟键码不可混用。
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub mod compile;
pub mod recording;
#[cfg(test)]
mod tests;
pub type Result<T> = std::result::Result<T, String>;
pub const PAUSE_SCAN_CODE: u16 = 0xe11d;
pub fn original_scan_code(raw: u32) -> Result<u16> {
    match raw {
        0xe11d45 | 0xe08a => Ok(PAUSE_SCAN_CODE),
        _ => u16::try_from(raw).map_err(|_| "原厂扫描码超出已核对的SDK范围".into()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Device {
    Keyboard,
    Mouse,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    pub devices: Vec<DeviceSettings>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceSettings {
    pub device: Device,
    pub enabled: bool,
    pub bindings: Vec<Binding>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub trigger: u16,
    pub action: Action,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    Keys {
        steps: Vec<Step>,
    },
    Text {
        text: String,
        before: Vec<Click>,
        after: Vec<Click>,
    },
    Launch {
        path: String,
        event_id: u16,
    },
    Disable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Step {
    Key {
        scan_code: u16,
        pressed: bool,
        delay_after_ms: u32,
    },
    MouseClick {
        button: u16,
        make_delay_ms: u32,
        break_delay_ms: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Click {
    pub scan_code: u16,
    pub make_delay_ms: u32,
    pub break_delay_ms: u32,
}

pub fn validate_code(device: Device, code: u16) -> Result<()> {
    let valid = match device {
        Device::Keyboard => code != 0 && code != 0xe077 && code != 0xe078,
        Device::Mouse => [1, 4, 16, 64, 256].contains(&code),
    };
    if !valid {
        return Err("扫描码或鼠标按键编码无效".into());
    }
    Ok(())
}
pub fn validate(configuration: &Configuration) -> Result<()> {
    if configuration.devices.is_empty() {
        return Err("没有选择映射设备".into());
    }
    let mut devices = BTreeSet::new();
    let mut events = BTreeSet::new();
    for device in &configuration.devices {
        if !devices.insert(device.device) {
            return Err("映射设备重复".into());
        }
        let mut triggers = BTreeSet::new();
        for binding in &device.bindings {
            validate_code(device.device, binding.trigger)?;
            if !triggers.insert(binding.trigger) {
                return Err("同一按键配置了多个动作".into());
            }
            validate_action(&binding.action)?;
            if let Action::Launch { event_id, .. } = binding.action {
                if !events.insert(event_id) {
                    return Err("程序启动事件编号重复".into());
                }
            }
        }
    }
    Ok(())
}
fn validate_action(action: &Action) -> Result<()> {
    match action {
        Action::Keys { steps } => validate_steps(steps),
        Action::Text {
            text,
            before,
            after,
        } => {
            if text.is_empty() || text.contains('\0') {
                return Err("文本宏不能为空或包含空字符".into());
            }
            if before.len() > 6 || after.len() > 6 {
                return Err("文本宏前后组合各最多6个按键".into());
            }
            for click in before.iter().chain(after) {
                validate_code(Device::Keyboard, click.scan_code)?;
            }
            Ok(())
        }
        Action::Launch { path, event_id } => {
            if path.is_empty() || path.contains('\0') || *event_id < 256 {
                return Err("程序路径或事件编号无效".into());
            }
            Ok(())
        }
        Action::Disable => Ok(()),
    }
}
pub fn validate_steps(steps: &[Step]) -> Result<()> {
    if steps.is_empty() || steps.len() > 256 {
        return Err("按键宏须包含1–256个步骤".into());
    }
    let mut held = BTreeSet::new();
    for step in steps {
        match *step {
            Step::Key {
                scan_code, pressed, ..
            } => {
                validate_code(Device::Keyboard, scan_code)?;
                if pressed {
                    held.insert(scan_code);
                } else if !held.remove(&scan_code) {
                    return Err("宏中存在尚未按下的释放动作".into());
                }
            }
            Step::MouseClick { button, .. } => validate_code(Device::Mouse, button)?,
        }
    }
    if !held.is_empty() {
        return Err("宏结束前须释放所有按键".into());
    }
    Ok(())
}
