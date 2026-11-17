//! 只解析系统厂商；不保留序列号或设备UUID。
use anyhow::{Context, Result, bail};

pub(crate) fn manufacturer(raw: &[u8]) -> Result<String> {
    if raw.len() < 8 || raw.len() > 1_048_576 {
        bail!("SMBIOS数据长度无效");
    }
    let length = u32::from_le_bytes(raw[4..8].try_into()?) as usize;
    if length != raw.len() - 8 {
        bail!("SMBIOS声明长度不一致");
    }
    let table = &raw[8..];
    let mut at = 0;
    let mut result = None;
    for _ in 0..4096 {
        if at == table.len() {
            return result.context("SMBIOS缺少系统厂商");
        }
        let header = table.get(at..at + 4).context("SMBIOS结构头不完整")?;
        let length = usize::from(header[1]);
        if length < 4 {
            bail!("SMBIOS结构长度无效");
        }
        let formatted = table
            .get(at..at + length)
            .context("SMBIOS格式化结构不完整")?;
        let strings = table.get(at + length..).context("SMBIOS字符串区缺失")?;
        let end = strings
            .windows(2)
            .position(|pair| pair == [0, 0])
            .context("SMBIOS字符串区缺少结束符")?;
        if header[0] == 1 {
            if result.is_some() || formatted.len() < 8 {
                bail!("SMBIOS系统结构重复或无效");
            }
            let index = usize::from(formatted[4]);
            if index == 0 {
                bail!("SMBIOS系统厂商未报告");
            }
            let bytes = strings[..end + 1]
                .split(|value| *value == 0)
                .nth(index - 1)
                .filter(|value| !value.is_empty())
                .context("SMBIOS系统厂商索引无效")?;
            let value = std::str::from_utf8(bytes).context("SMBIOS系统厂商文本无效")?;
            if value.len() > 4096 {
                bail!("SMBIOS系统厂商文本过长");
            }
            result = Some(value.to_owned());
        }
        at += length + end + 2;
        if header[0] == 127 {
            if at != table.len() && table[at..].iter().any(|v| *v != 0) {
                bail!("SMBIOS结束结构后仍有未知数据");
            }
            return result.context("SMBIOS缺少系统厂商");
        }
    }
    bail!("SMBIOS结构数量超过支持范围")
}

#[cfg(not(test))]
pub(crate) fn read_manufacturer() -> Result<String> {
    use windows_sys::Win32::System::SystemInformation::GetSystemFirmwareTable;
    let size = unsafe { GetSystemFirmwareTable(0x52534D42, 0, std::ptr::null_mut(), 0) };
    if !(8..=1_048_576).contains(&size) {
        bail!("无法取得SMBIOS表大小：{size}");
    }
    let mut raw = vec![0; size as usize];
    let read = unsafe { GetSystemFirmwareTable(0x52534D42, 0, raw.as_mut_ptr(), size) };
    if read != size {
        bail!("SMBIOS表在读取期间变化或读取失败");
    }
    manufacturer(&raw)
}

impl super::Hal {
    pub(crate) fn manufacturer(&self) -> Result<String> {
        #[cfg(test)]
        {
            self.test_manufacturer
                .clone()
                .ok_or_else(|| anyhow::anyhow!("隔离SMBIOS厂商未配置，禁止回退系统查询"))
        }
        #[cfg(not(test))]
        {
            read_manufacturer()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn raw(table: &[u8]) -> Vec<u8> {
        let mut data = vec![0, 3, 6, 0];
        data.extend_from_slice(&(table.len() as u32).to_le_bytes());
        data.extend_from_slice(table);
        data
    }
    #[test]
    fn system_type_and_string_index_select_the_manufacturer_only() {
        let table = [
            1, 8, 0, 0, 2, 1, 0, 0, b'X', 0, b'M', b'O', b'N', b'S', b'T', b'E', b'R', 0, 0, 127,
            4, 0, 0, 0, 0,
        ];
        assert_eq!(manufacturer(&raw(&table)).unwrap(), "MONSTER");
        for end in 0..table.len() - 6 {
            assert!(manufacturer(&raw(&table[..end])).is_err());
        }
    }
    #[test]
    fn malformed_duplicate_missing_and_unknown_strings_are_not_defaulted() {
        let good = [1, 8, 0, 0, 1, 0, 0, 0, b'A', 0, 0];
        assert!(manufacturer(&raw(&[good.as_slice(), good.as_slice()].concat())).is_err());
        for index in [0, 2, 255] {
            let mut bad = good;
            bad[4] = index;
            assert!(manufacturer(&raw(&bad)).is_err());
        }
        let mut bad = raw(&good);
        bad[4] += 1;
        assert!(manufacturer(&bad).is_err());
    }
}
