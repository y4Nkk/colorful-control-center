use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Click {
        kind: i32,
        code: u16,
        make_delay_ms: u32,
        break_delay_ms: u32,
    },
    Down {
        code: u16,
        delay_ms: u32,
    },
    Up {
        code: u16,
        delay_ms: u32,
    },
    Text(Vec<u16>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledBinding {
    pub hotkeys: Vec<Vec<Item>>,
    pub actions: Vec<Item>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledDevice {
    pub device: Device,
    pub enabled: bool,
    pub bindings: Vec<CompiledBinding>,
}

fn click(kind: i32, code: u16, make_delay_ms: u32, break_delay_ms: u32) -> Item {
    Item::Click {
        kind,
        code,
        make_delay_ms,
        break_delay_ms,
    }
}
fn clicks(keys: &[Click]) -> Vec<Item> {
    keys.iter()
        .map(|key| click(0, key.scan_code, key.make_delay_ms, key.break_delay_ms))
        .collect()
}
fn actions(action: &Action) -> Vec<Item> {
    match action {
        Action::Keys { steps } => steps
            .iter()
            .map(|step| match *step {
                Step::Key {
                    scan_code,
                    pressed: true,
                    delay_after_ms,
                } => Item::Down {
                    code: scan_code,
                    delay_ms: delay_after_ms,
                },
                Step::Key {
                    scan_code,
                    pressed: false,
                    delay_after_ms,
                } => Item::Up {
                    code: scan_code,
                    delay_ms: delay_after_ms,
                },
                Step::MouseClick {
                    button,
                    make_delay_ms,
                    break_delay_ms,
                } => click(1, button, make_delay_ms, break_delay_ms),
            })
            .collect(),
        Action::Text {
            text,
            before,
            after,
        } => {
            let mut items = clicks(before);
            items.push(Item::Text(text.encode_utf16().chain([0]).collect()));
            items.extend(clicks(after));
            items
        }
        Action::Launch { event_id, .. } => vec![click(3, *event_id, 0, 0)],
        Action::Disable => vec![],
    }
}
fn hotkeys(device: Device, code: u16) -> Vec<Vec<Item>> {
    match device {
        Device::Mouse => vec![vec![click(1, code, 0, 0)]],
        Device::Keyboard => vec![
            vec![click(0, code, 0, 0)],
            vec![click(0, 0xe077, 0, 0), click(0, code, 0, 0)],
            vec![click(0, 0xe078, 0, 0), click(0, code, 0, 0)],
        ],
    }
}
pub fn configuration(config: &Configuration) -> Result<Vec<CompiledDevice>> {
    validate(config)?;
    Ok(config
        .devices
        .iter()
        .map(|device| CompiledDevice {
            device: device.device,
            enabled: device.enabled,
            bindings: device
                .bindings
                .iter()
                .map(|binding| CompiledBinding {
                    hotkeys: hotkeys(device.device, binding.trigger),
                    actions: actions(&binding.action),
                })
                .collect(),
        })
        .collect())
}
