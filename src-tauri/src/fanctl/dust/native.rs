use super::*;
use crate::hal::oem_dchu::Dchu;

impl Io for Dchu {
    fn running(&self) -> Result<Option<bool>> {
        self.dust_running()
    }
    fn fan_mode(&self) -> Result<u8> {
        self.read_fan_mode().context("无法确认除尘前的风扇档位")
    }
    fn duties(&self) -> Result<[f64; 2]> {
        let packet = self.read_package(12)?;
        if packet.iter().all(|b| *b == 0) || packet.iter().all(|b| *b == 255) {
            bail!("无法确认当前风扇占空比");
        }
        Ok([packet[16], packet[19]].map(|v| f64::from(v) * 100.0 / 255.0))
    }
    fn sync_clock(&self) -> Result<()> {
        let mut now = windows_sys::Win32::Foundation::SYSTEMTIME::default();
        unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut now) };
        let value = protocol::clock(now.wDayOfWeek, now.wHour, now.wMinute, now.wSecond)?;
        self.set_data_sub(118, 1, value)
    }
    fn command(&self, sub: u8, value: u32) -> Result<()> {
        self.set_data_sub(121, sub, value)
    }
}
