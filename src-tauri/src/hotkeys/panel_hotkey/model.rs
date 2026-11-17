use crate::hal::panel_inventory::Inventory;
use anyhow::{Context, Result, bail};
use ccc_core::panel_hotkey::{Kind, pointer_wake_allowed};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Saver {
    pub path: PathBuf,
    pub fingerprint: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Source {
    pub kind: Kind,
    pub inventory: Inventory,
    pub hook_flags: Option<[u8; 3]>,
    pub saver: Option<Saver>,
}
impl Source {
    pub fn new(inventory: Inventory, flags: Option<&[u8]>, saver: Option<Saver>) -> Result<Self> {
        if inventory.monitors.iter().any(|monitor| monitor.oled) {
            let saver = saver.context("Windows空白屏保文件不可用")?;
            if !saver.path.is_absolute()
                || saver.fingerprint.len() != 64
                || !saver.fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
            {
                bail!("Windows空白屏保身份无效");
            }
            return Ok(Self {
                kind: Kind::BlankSaver,
                inventory,
                hook_flags: None,
                saver: Some(saver),
            });
        }
        if inventory.monitors.is_empty() || !inventory.unresolved.is_empty() {
            bail!("显示面板身份未完整确认，无法选择原厂非OLED行为");
        }
        let flags = flags.context("原厂屏幕按键能力未提供")?;
        if !pointer_wake_allowed(flags).map_err(anyhow::Error::msg)? {
            bail!("固件已禁止原厂屏幕按键的输入唤醒监听");
        }
        let disabled = if flags[0] == 1 { flags[15] & 8 } else { 0 };
        Ok(Self {
            kind: Kind::PointerWake,
            inventory,
            hook_flags: Some([flags[0], flags[1], disabled]),
            saver: None,
        })
    }
    pub fn identity(&self) -> Result<String> {
        super::super::model::digest(self)
    }
    pub fn require(&self, identity: &str) -> Result<()> {
        if self.identity()? != identity {
            bail!("屏幕按键适用条件或来源变化，请重新确认");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Owned {
    pub id: u64,
    pub identity: String,
    pub kind: Kind,
    pub preparing: bool,
    pub error: Option<String>,
}
impl Owned {
    pub fn revision(&self) -> Result<String> {
        super::super::model::digest(self)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub identity: Option<String>,
    pub current: Option<Source>,
    pub error: Option<String>,
    pub session: Option<Owned>,
    pub restore_revision: Option<String>,
}
