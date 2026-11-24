use super::*;

impl Dchu {
    /// 原厂page7是cmd4/sub8的缓存；自研直接查询，不依赖原厂先运行。
    pub fn read_bios_capabilities(&self, offset: usize, length: usize) -> Result<Vec<u8>> {
        let index = i32::try_from(offset).map_err(|_| anyhow!("BIOS能力字段越界"))?;
        validate_settings_range(7, index, length)?;
        let packet = self.bios_capabilities()?;
        Ok(packet[offset..offset + length].to_vec())
    }
    pub fn bios_capabilities(&self) -> Result<[u8; 256]> {
        let mut request = [0; 256];
        request[0] = 8;
        let packet = self.exchange_package(4, &request)?;
        if packet.iter().all(|byte| *byte == 0) {
            return Err(anyhow!("固件返回空的 BIOS 能力包"));
        }
        if !matches!(&packet[..2], [0, 0] | [1, 0]) {
            return Err(anyhow!("未知BIOS能力版本{}.{}", packet[0], packet[1]));
        }
        Ok(packet)
    }

    /// None 表示已确认不支持除尘。包或整数无效时，能力和运行态均未知。
    pub fn dust_running(&self) -> Result<Option<bool>> {
        use anyhow::Context;

        let caps = self.bios_capabilities().context("无法确认固件除尘能力")?;
        let flags = if caps[0] == 1 {
            i32::from(u16::from_le_bytes([caps[10], caps[11]]))
        } else {
            let (valid, flags) = self.read_int(96);
            if !valid || flags == -1 {
                return Err(anyhow!("原厂 WMI96 除尘能力不可读"));
            }
            flags
        };
        if flags & 0x80 == 0 {
            return Ok(None);
        }

        const COMMAND: u8 = 0xB8;
        let mut packet = [0; 256];
        packet[..7].copy_from_slice(&[1, 0xA3, 0, 0, 0, 0, COMMAND]);
        let reply = self
            .exchange_package(4, &packet)
            .context("无法读取固件除尘状态")?;
        // PK04 原样返回 FCMD 和 FDAT。单独的全零响应无法证明 EC 空闲。
        match (reply[0], reply[1]) {
            (COMMAND, 0) => Ok(Some(false)),
            (COMMAND | 0, 1) => Ok(Some(true)),
            (command, state) => Err(anyhow!(
                "固件除尘状态未确认：FCMD={command:#04x}，FDAT={state:#04x}"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "system" fn unused_packet(_: i32, _: *mut u8) -> i32 {
        0
    }
    unsafe extern "system" fn unused_read(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
        panic!("能力查询不得读取原厂缓存");
    }
    unsafe extern "system" fn unused_set(_: i32, _: *const u8, _: i32) -> i32 {
        0
    }
    unsafe extern "system" fn unused_write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
        panic!("能力查询不得写入共享缓存");
    }
    unsafe extern "system" fn firmware(cmd: i32, input: *const u8, len: i32, out: *mut u8) -> i32 {
        assert_eq!((cmd, len), (4, 256));
        assert_eq!(unsafe { std::slice::from_raw_parts(input, 256) }, {
            let mut expected = [0u8; 256];
            expected[0] = 8;
            expected
        });
        unsafe {
            std::ptr::write_bytes(out, 0, 257);
            out.write(1);
            out.add(17).write(15);
            out.add(16).write(16);
            out.add(256).write(1);
        }
        4
    }
    unsafe extern "system" fn failed(_: i32, _: *const u8, _: i32, _: *mut u8) -> i32 {
        0
    }
    unsafe extern "system" fn empty(_: i32, _: *const u8, _: i32, _: *mut u8) -> i32 {
        4
    }
    unsafe extern "system" fn zeroed(_: i32, _: *const u8, _: i32, out: *mut u8) -> i32 {
        unsafe { std::ptr::write_bytes(out, 0, 257) };
        4
    }
    unsafe extern "system" fn unknown(cmd: i32, input: *const u8, len: i32, out: *mut u8) -> i32 {
        let status = unsafe { firmware(cmd, input, len, out) };
        unsafe {
            out.write(2);
            out.add(256).write(2);
        }
        status
    }

    fn io(exchange: SetDataEx) -> Dchu {
        Dchu::test_io(unused_packet, unused_read, unused_set, unused_write)
            .with_test_exchange(exchange)
    }
    #[test]
    fn live_capabilities_replace_cache_for_direct_and_logical_reads() {
        use ccc_core::settings::SettingsAccess;
        let io = io(firmware);
        assert_eq!(io.read_bios_capabilities(16, 2).unwrap(), [16, 15]);
        assert_eq!(io.read(7, 16, 2).unwrap(), [16, 15]);
        assert!(io.read_settings(7, 16, 2).is_err());
    }
    #[test]
    fn unavailable_or_empty_response_never_falls_back_to_saved_capabilities() {
        assert!(io(failed).read_bios_capabilities(0, 24).is_err());
        assert!(io(empty).read_bios_capabilities(0, 24).is_err());
        assert!(
            io(zeroed)
                .bios_capabilities()
                .unwrap_err()
                .to_string()
                .contains("空")
        );
        assert!(io(firmware).read_bios_capabilities(255, 2).is_err());
        assert!(io(firmware).read_bios_capabilities(usize::MAX, 1).is_err());
        assert!(io(unknown).bios_capabilities().is_err());
    }
}
