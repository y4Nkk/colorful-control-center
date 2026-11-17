use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mode {
    pub device: String,
    pub identity: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub bits: u32,
    pub orientation: u32,
    pub x: i32,
    pub y: i32,
    pub flags: u32,
    pub frequency: u32,
}
impl Mode {
    pub fn validate(&self) -> Result<()> {
        if self.device.is_empty()
            || self.device.len() > 512
            || self.device.contains('\0')
            || self.identity.is_empty()
            || self.identity.len() > 2048
            || self.identity.contains('\0')
            || self.name.len() > 1024
            || self.name.contains('\0')
            || self.width == 0
            || self.height == 0
            || self.bits == 0
            || self.orientation > 3
            || self.frequency < 2
        {
            bail!("显示目标包含无效的设备身份或模式");
        }
        Ok(())
    }
    pub fn same_state(&self, other: &Self) -> bool {
        self.device == other.device
            && self.identity == other.identity
            && self.width == other.width
            && self.height == other.height
            && self.bits == other.bits
            && self.orientation == other.orientation
            && self.x == other.x
            && self.y == other.y
            && self.flags == other.flags
            && self.frequency == other.frequency
    }
    pub fn oem_rotation(&self) -> Result<Self> {
        self.validate()?;
        let mut target = self.clone();
        match self.orientation {
            0 => target.orientation = 2,
            2 => target.orientation = 0,
            1 | 3 => {
                std::mem::swap(&mut target.width, &mut target.height);
                target.orientation = 2;
            }
            _ => bail!("显示方向无效"),
        }
        Ok(target)
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    pub current: Mode,
    pub frequencies: Vec<u32>,
    pub primary: bool,
}
pub trait Access: Send + Sync {
    fn list(&self) -> Result<Vec<Display>>;
    fn test(&self, mode: &Mode) -> Result<()>;
    fn set(&self, mode: &Mode) -> Result<()>;
}
