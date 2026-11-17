//! 探测 CLI 复用桌面 HAL 的 PawnIO 传输与 ccc-core 唯一 MSR 协议。
#[path = "../../../src-tauri/src/hal/pawnio.rs"]
mod transport;
use anyhow::{Result, anyhow};
use ccc_core::msr;
pub use transport::PawnIo;

pub const PLANES: [(&str, u64); 5] = [
    ("cpu", 0),
    ("gpu", 1),
    ("cache", 2),
    ("uncore", 3),
    ("analogio", 4),
];

pub fn read_voltage_offset(io: &PawnIo, plane: u64) -> Result<f64> {
    msr::read_voltage_units(io, plane)
        .map(|v| v as f64 / 1.024)
        .map_err(|e| anyhow!(e))
}

/// 无论探测回读成功与否，按原始编码恢复，再判断一编码步进是否真实生效。
pub fn test_voltage_write(io: &PawnIo, plane: u64) -> Result<msr::VoltageProbe> {
    msr::probe_voltage_write(io, plane).map_err(|e| anyhow!(e))
}

pub fn decode_pkg_power_limit(value: u64, rapl: u64) -> String {
    let unit = msr::power_unit(rapl);
    let (pl1, pl2) = msr::decode_power_limits(value, unit);
    format!(
        "PL1={pl1:.3}W PL2={pl2:.3}W | RAPL单位={unit}W | LOCK(bit63)={}",
        value >> 63
    )
}
