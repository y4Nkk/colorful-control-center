use anyhow::{Context, Result, anyhow, bail};
use ccc_core::keymap::{self, Action, Binding, Click, Configuration, Device, DeviceSettings, Step};
use roxmltree::{Document, Node};
use sha2::{Digest, Sha256};
use std::{path::Path, str::FromStr};
#[cfg(test)]
#[path = "original/tests.rs"]
mod tests;

#[derive(Clone)]
pub struct Source {
    pub name: String,
    pub configuration: Configuration,
    pub fingerprint: String,
}
fn attribute(node: Node<'_, '_>, name: &str) -> Result<String> {
    node.attribute(name)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("原厂宏配置缺少{name}字段"))
}
fn number<T: FromStr>(node: Node<'_, '_>, name: &str) -> Result<T> {
    attribute(node, name)?
        .parse()
        .map_err(|_| anyhow!("原厂宏配置的{name}字段无效"))
}
fn flag(node: Node<'_, '_>, name: &str) -> Result<bool> {
    match number::<u8>(node, name)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => bail!("原厂宏配置的{name}开关无效"),
    }
}
#[derive(Clone)]
struct Event {
    code: u16,
    pressed: bool,
    delay: u32,
}
fn event(node: Node<'_, '_>) -> Result<Event> {
    let pressed = match attribute(node, "KeyMakeBreak")?.as_str() {
        "KeyMake" => true,
        "KeyBreak" => false,
        _ => bail!("原厂宏按下/释放类型无效"),
    };
    Ok(Event {
        code: keymap::original_scan_code(number(node, "KeyCode")?).map_err(|e| anyhow!(e))?,
        pressed,
        delay: number(node, "KeyDelay")?,
    })
}
fn steps(events: &[Event], timed: bool) -> Vec<Step> {
    events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.code != 0)
        .map(|(index, event)| Step::Key {
            scan_code: event.code,
            pressed: event.pressed,
            delay_after_ms: if timed {
                events.get(index + 1).map_or(0, |event| event.delay)
            } else {
                0
            },
        })
        .collect()
}
fn clicks(events: &[Event]) -> Result<Vec<Click>> {
    let nonzero: Vec<_> = events
        .iter()
        .filter(|event| event.code != 0)
        .cloned()
        .collect();
    if !nonzero.is_empty() {
        keymap::validate_steps(&steps(&nonzero, false)).map_err(|e| anyhow!(e))?;
    }
    Ok(nonzero
        .iter()
        .filter(|event| event.pressed)
        .map(|event| Click {
            scan_code: event.code,
            make_delay_ms: 0,
            break_delay_ms: 0,
        })
        .collect())
}
fn keys(node: Node<'_, '_>) -> Result<Action> {
    let timed = flag(node, "IsKeypressDelay")?;
    let events = node
        .children()
        .filter(Node::is_element)
        .map(|child| {
            if !child.tag_name().name().starts_with("Index_") {
                bail!("原厂按键宏包含未知步骤");
            }
            event(child)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Action::Keys {
        steps: steps(&events, timed),
    })
}
fn text(node: Node<'_, '_>) -> Result<Action> {
    let mut before = vec![];
    let mut after = vec![];
    let mut content = None;
    for child in node.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "StartKey" => before.push(event(child)?),
            "SendKey" => after.push(event(child)?),
            "Text" if content.is_none() => content = Some(attribute(child, "TextContent")?),
            _ => bail!("原厂文本宏包含未知或重复内容"),
        }
    }
    Ok(Action::Text {
        text: content.context("原厂文本宏缺少内容")?,
        before: clicks(&before)?,
        after: clicks(&after)?,
    })
}
fn binding(node: Node<'_, '_>) -> Result<Option<(Device, Binding)>> {
    let action = match number::<u8>(node, "Action")? {
        0 => return Ok(None),
        1 => keys(node)?,
        3 => text(node)?,
        4 => Action::Disable,
        2 => {
            let children: Vec<_> = node.children().filter(Node::is_element).collect();
            if children.len() != 1 || !children[0].tag_name().name().starts_with("Index_") {
                bail!("原厂程序宏内容无效");
            }
            Action::Launch {
                path: attribute(children[0], "AppPath")?,
                event_id: number(children[0], "AppEventId")?,
            }
        }
        _ => bail!("原厂宏动作类型未识别"),
    };
    let device = match number::<u8>(node, "Device")? {
        1 => Device::Keyboard,
        2 => Device::Mouse,
        _ => bail!("原厂宏设备类型无效"),
    };
    Ok(Some((
        device,
        Binding {
            trigger: match device {
                Device::Keyboard => {
                    keymap::original_scan_code(number(node, "KeyCode")?).map_err(|e| anyhow!(e))?
                }
                Device::Mouse => number(node, "KeyCode")?,
            },
            action,
        },
    )))
}
pub fn decode(bytes: &[u8]) -> Result<Source> {
    let xml = std::str::from_utf8(bytes)
        .context("原厂宏配置不是UTF-8")?
        .trim_start_matches('\u{feff}');
    let doc = Document::parse(xml).context("原厂宏XML配置损坏")?;
    let root = doc.root_element();
    if root.tag_name().name() != "root" {
        bail!("原厂宏配置根节点无效");
    }
    let mut selected = None;
    for node in root.children().filter(Node::is_element) {
        if !node.tag_name().name().starts_with("Profile_") {
            bail!("原厂宏配置包含未知配置节点");
        }
        if flag(node, "IsSelected")? {
            if selected.replace(node).is_some() {
                bail!("原厂宏存在多个活动配置");
            }
        }
    }
    let node = selected.context("原厂宏没有活动配置，不能猜测恢复基线")?;
    let mut keyboard = DeviceSettings {
        device: Device::Keyboard,
        enabled: flag(node, "IsKbEnable")?,
        bindings: vec![],
    };
    let mut mouse = DeviceSettings {
        device: Device::Mouse,
        enabled: flag(node, "IsMouseEnable")?,
        bindings: vec![],
    };
    for child in node.children().filter(Node::is_element) {
        if !child.tag_name().name().starts_with("Set_") {
            bail!("原厂活动配置包含未知设置节点");
        }
        if let Some((device, binding)) = binding(child)? {
            match device {
                Device::Keyboard => keyboard.bindings.push(binding),
                Device::Mouse => mouse.bindings.push(binding),
            }
        }
    }
    let configuration = Configuration {
        devices: vec![keyboard, mouse],
    };
    keymap::validate(&configuration).map_err(|e| anyhow!(e))?;
    Ok(Source {
        name: attribute(node, "Name")?,
        configuration,
        fingerprint: format!("{:x}", Sha256::digest(bytes)),
    })
}
pub fn read(path: &Path) -> Result<Option<Source>> {
    match std::fs::read(path) {
        Ok(bytes) => decode(&bytes)
            .map(Some)
            .with_context(|| format!("原厂宏配置无法解析（文件指纹{:x}）", Sha256::digest(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("无法读取原厂宏配置"),
    }
}
