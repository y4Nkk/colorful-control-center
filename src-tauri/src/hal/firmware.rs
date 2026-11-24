//! 只读识别已审查的 DSDT；各领域仍须分别核对该固件的对应方法体。
use super::oem_dchu::Dchu;
use anyhow::{Result, anyhow, ensure};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use windows_sys::Win32::System::SystemInformation::GetSystemFirmwareTable;
pub(crate) const DIRECT_FAN_MASK: u32 = 3;
pub(crate) const FAN_CONTROL_UNVERIFIED: &str = "当前固件无法核实逐通道风扇控制权";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectControl {
    Firmware,
    Manual(u8),
}

pub(crate) type ReadDirectControl = fn(&Dchu) -> Result<[DirectControl; 2]>;

// The reviewed DSDT exposes the mailbox, instantaneous duty and preset, but no ownership readback.
pub(crate) const DIRECT_FAN_READER: Option<ReadDirectControl> = None;

pub(crate) fn verify_direct_release(dchu: &Dchu, read: Option<ReadDirectControl>) -> Result<()> {
    let read = read.ok_or_else(|| anyhow!(FAN_CONTROL_UNVERIFIED))?;
    ensure!(
        read(dchu)? == [DirectControl::Firmware; 2],
        "EC 仍有风扇通道处于软件控制"
    );
    Ok(())
}

const TARGET: [u8; 32] = [
    0xd5, 0x16, 0x2a, 0x4a, 0x00, 0x25, 0x76, 0xad, 0x9d, 0xcc, 0x77, 0x11, 0x50, 0x01, 0xfb, 0xc6,
    0x68, 0x01, 0x92, 0x7d, 0x78, 0x90, 0xc1, 0x31, 0x6b, 0x98, 0x39, 0xa3, 0xc1, 0x4b, 0x52, 0x03,
];

pub fn reviewed_dsdt() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        let mut table = vec![0u8; 551584];
        let size = unsafe {
            GetSystemFirmwareTable(
                0x41435049,
                0x54445344,
                table.as_mut_ptr(),
                table.len() as u32,
            )
        };
        size as usize == table.len() && Sha256::digest(&table)[..] == TARGET
    })
}
