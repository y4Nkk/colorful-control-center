use super::Dchu;
use anyhow::{Result, anyhow, ensure};
use ccc_core::fan::{decode_oem_fan_mode, oem_fan_mode_bits};

impl Dchu {
    pub fn read_fan_info(&self) -> Result<[u8; 256]> {
        let info = self.read_package(13)?;
        ensure!((1..=4).contains(&info[12]), "EC 风扇能力包未报告有效通道数");
        Ok(info)
    }

    pub fn read_fan_mode(&self) -> Result<u8> {
        decode_oem_fan_mode(self.read_fan_info()?[14]).map_err(anyhow::Error::msg)
    }

    pub fn set_fan_mode_verified(&self, mode: u8) -> Result<()> {
        ensure!(oem_fan_mode_bits(mode).is_some(), "不支持此风扇档位 {mode}");
        self.set_data_sub(121, 1, u32::from(mode))?;
        let actual = self.read_fan_mode()?;
        ensure!(actual == mode, "EC 风扇档位回读为 {actual}，目标为 {mode}");
        Ok(())
    }

    pub fn read_fan_offset(&self, reviewed_dsdt: bool) -> Result<u8> {
        self.read_fan_mode()?;
        Ok(self.fan_window(3, reviewed_dsdt)?[0xe7])
    }

    pub fn set_fan_offset_verified(&self, reviewed_dsdt: bool, raw: u8) -> Result<()> {
        ensure!(reviewed_dsdt, "当前固件未确认 EC 风扇偏移回读通道");
        self.set_data_sub(121, 14, u32::from(raw))?;
        let actual = self.read_fan_offset(reviewed_dsdt)?;
        ensure!(actual == raw, "EC 风扇偏移回读为 {actual}，目标为 {raw}");
        Ok(())
    }

    pub(crate) fn read_fan_curve_package(
        &self,
        reviewed_dsdt: bool,
        info: &[u8; 256],
        channels: u8,
    ) -> Result<[u8; 256]> {
        let window = self.fan_window(5, reviewed_dsdt)?;
        curve_package(&window, info, channels)
    }

    fn fan_window(&self, selector: u8, reviewed_dsdt: bool) -> Result<[u8; 256]> {
        ensure!(reviewed_dsdt, "当前固件未确认 EC 风扇只读窗口");
        let mut request = [0; 256];
        request[0] = selector;
        let data = self.exchange_package(4, &request)?;
        if data.iter().all(|&b| b == 0) || data.iter().all(|&b| b == 0xff) {
            return Err(anyhow!("EC 风扇只读窗口返回空数据"));
        }
        Ok(data)
    }
}

fn curve_package(window: &[u8; 256], info: &[u8; 256], channels: u8) -> Result<[u8; 256]> {
    ensure!(window[0x0a] == info[12], "EC 两点曲线窗口的风扇数量不一致");
    let mut package = [0; 256];
    for index in 0..3 {
        let ec = 0x0c + index * 14;
        let at = 16 + index * 8;
        let points = [
            window[ec + 1],
            window[ec + 5],
            window[ec + 2],
            window[ec + 6],
        ];
        if channels & (1 << index) != 0 {
            ensure!(
                window[ec] == info[at]
                    && window[ec + 4] == info[at + 1]
                    && info[at + 2..at + 6] == points,
                "EC 风扇 {} 的曲线回读不一致",
                index + 1
            );
        }
        let p = 2 + index * 4;
        let s = 14 + index * 6;
        package[p..p + 4].copy_from_slice(&points);
        package[s..s + 6].copy_from_slice(&window[ec + 8..ec + 14]);
    }
    Ok(package)
}
