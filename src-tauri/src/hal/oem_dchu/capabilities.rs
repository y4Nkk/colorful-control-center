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
        if !matches!(&packet[..2], [0, 0] | [1, 0]) {
            return Err(anyhow!("未知BIOS能力版本{}.{}", packet[0], packet[1]));
        }
        Ok(packet)
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
        assert!(io(firmware).read_bios_capabilities(255, 2).is_err());
        assert!(io(firmware).read_bios_capabilities(usize::MAX, 1).is_err());
        assert!(io(unknown).bios_capabilities().is_err());
    }
}
