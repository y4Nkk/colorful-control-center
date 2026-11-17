use super::*;
use sha2::{Digest, Sha256};

pub(super) struct Capabilities {
    pub pd: bool,
    pub deep_s5: bool,
}
pub(super) fn capabilities(bytes: &[u8]) -> Result<Capabilities> {
    if bytes.len() != 25 {
        bail!("平台能力页长度无效");
    }
    match &bytes[..2] {
        [0, 0] => Ok(Capabilities {
            pd: false,
            deep_s5: false,
        }),
        [1, 0] => Ok(Capabilities {
            pd: bytes[23] & 2 != 0,
            deep_s5: bytes[24] & 1 != 0,
        }),
        _ => bail!("未知平台能力页版本"),
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Snapshot {
    level: [u8; 2],
    table: [u8; 20],
    pub levels: Vec<u8>,
}
impl Snapshot {
    pub fn decode(mut level: [u8; 2], mut table: [u8; 20]) -> Result<Self> {
        let version = u16::from_be_bytes([table[0], table[1]]);
        let levels = match version {
            0 if level[1] == 0 => vec![0, 1],
            0 => (1..=level[1]).collect(),
            256 => {
                if table[16..20].iter().any(|bits| bits & !15 != 0) {
                    bail!("PD能力表包含未知等级位");
                }
                let bucket = match table[2] {
                    0..=59 => 16,
                    60..=89 => 17,
                    90..=99 => 18,
                    _ => 19,
                };
                (1..=4)
                    .filter(|level| table[bucket] & (1 << (level - 1)) != 0)
                    .collect()
            }
            _ => bail!("未知PD等级协议版本"),
        };
        if levels.is_empty() {
            bail!("当前适配器未声明可用PD档位");
        }
        if version == 0 {
            table.fill(0);
        } else {
            table[3..16].fill(0);
            level[1] = 0;
        }
        Ok(Self {
            level,
            table,
            levels,
        })
    }
    pub fn level(&self) -> u8 {
        self.level[0]
    }
    pub fn zero_based(&self) -> bool {
        self.table[..2] == [0, 0] && self.level[1] == 0
    }
    pub fn adapter_watts(&self) -> Option<u8> {
        (self.table[..2] == [1, 0] && self.table[2] != 0).then_some(self.table[2])
    }
    pub fn adapter_protocol(&self) -> bool {
        self.table[..2] == [1, 0]
    }
    pub fn validate_current(&self) -> Result<()> {
        if !self.levels.contains(&self.level()) {
            bail!("当前PD档位不在已声明范围，不能保证初始恢复");
        }
        Ok(())
    }
    pub fn revision(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(self).expect("固定PD事实"))
        )
    }
    pub fn target(&self, level: u8) -> Result<Self> {
        if !self.levels.contains(&level) {
            bail!("固件未声明此PD档位");
        }
        let mut result = self.clone();
        result.level[0] = level;
        Ok(result)
    }
    pub fn restorable(&self, initial: &Self) -> Result<()> {
        self.same_encoding(initial)?;
        if !self.levels.contains(&initial.level()) {
            bail!("当前适配器不支持初始PD档位，请接回原适配器后恢复");
        }
        Ok(())
    }
    pub fn same_encoding(&self, initial: &Self) -> Result<()> {
        if self.table[..2] != initial.table[..2] || self.zero_based() != initial.zero_based() {
            bail!("PD等级编号语义已变化，拒绝旧快照");
        }
        Ok(())
    }
}
