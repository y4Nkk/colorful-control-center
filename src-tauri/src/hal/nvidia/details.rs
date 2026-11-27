use super::{Connection, Device, NvmlError, Query};
use anyhow::Result;
use nvml_wrapper::{
    bitmasks::device::ThrottleReasons, enum_wrappers::device::PerformanceState,
    enums::device::SampleValue, error::Bits, structs::device::FieldId,
    sys_exports::field_id::NVML_FI_DEV_MEMORY_TEMP,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GpuDetails {
    encoder_utilization_pct: Option<u32>,
    decoder_utilization_pct: Option<u32>,
    performance_state: Option<u32>,
    pcie_link_generation: Option<u32>,
    pcie_link_width: Option<u32>,
    pcie_max_link_generation: Option<u32>,
    pcie_max_link_width: Option<u32>,
    clock_limit_reasons: Option<Vec<ClockLimitReason>>,
    memory_temperature_c: Option<f64>,
    fan_target_rpm: Option<u32>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum ClockLimitReason {
    Idle,
    ApplicationClocks,
    PowerCap,
    HardwareSlowdown,
    SyncBoost,
    SoftwareTemperature,
    HardwareTemperature,
    PowerBrake,
    DisplayClocks,
    Unknown,
}

impl Connection {
    pub(super) fn details(&mut self, device: &Device<'_>) -> Result<GpuDetails> {
        let support = &mut self.support;
        Ok(GpuDetails {
            encoder_utilization_pct: support
                .read(Query::Encoder, || device.encoder_utilization())?
                .and_then(|value| (value.utilization <= 100).then_some(value.utilization)),
            decoder_utilization_pct: support
                .read(Query::Decoder, || device.decoder_utilization())?
                .and_then(|value| (value.utilization <= 100).then_some(value.utilization)),
            performance_state: support
                .read(Query::Performance, || device.performance_state())?
                .and_then(|state| (state != PerformanceState::Unknown).then_some(state as u32)),
            pcie_link_generation: support
                .read(Query::PcieGen, || device.current_pcie_link_gen())?
                .filter(|value| *value > 0),
            pcie_link_width: support
                .read(Query::PcieWidth, || device.current_pcie_link_width())?
                .filter(|value| *value > 0),
            pcie_max_link_generation: support
                .static_value(&mut self.max_link_gen, Query::PcieMaxGen, || {
                    device.max_pcie_link_gen()
                })?
                .filter(|value| *value > 0),
            pcie_max_link_width: support
                .static_value(&mut self.max_link_width, Query::PcieMaxWidth, || {
                    device.max_pcie_link_width()
                })?
                .filter(|value| *value > 0),
            clock_limit_reasons: support.read(Query::ClockLimits, || clock_limits(device))?,
            memory_temperature_c: support
                .read(Query::MemoryTemperature, || memory_temperature(device))?
                .flatten(),
            fan_target_rpm: support.read(Query::FanTargetRpm, || device.fan_speed_rpm(0))?,
        })
    }
}

fn clock_limits(device: &Device<'_>) -> Result<Vec<ClockLimitReason>, NvmlError> {
    match device.current_throttle_reasons_strict() {
        Ok(reasons) => Ok(decode_reasons(reasons, false)),
        Err(NvmlError::IncorrectBits(Bits::U64(bits))) => Ok(decode_reasons(
            ThrottleReasons::from_bits_truncate(bits),
            true,
        )),
        Err(error) => Err(error),
    }
}

fn decode_reasons(reasons: ThrottleReasons, unknown: bool) -> Vec<ClockLimitReason> {
    use ClockLimitReason::*;
    let mut result = [
        (ThrottleReasons::GPU_IDLE, Idle),
        (
            ThrottleReasons::APPLICATIONS_CLOCKS_SETTING,
            ApplicationClocks,
        ),
        (ThrottleReasons::SW_POWER_CAP, PowerCap),
        (ThrottleReasons::HW_SLOWDOWN, HardwareSlowdown),
        (ThrottleReasons::SYNC_BOOST, SyncBoost),
        (ThrottleReasons::SW_THERMAL_SLOWDOWN, SoftwareTemperature),
        (ThrottleReasons::HW_THERMAL_SLOWDOWN, HardwareTemperature),
        (ThrottleReasons::HW_POWER_BRAKE_SLOWDOWN, PowerBrake),
        (ThrottleReasons::DISPLAY_CLOCK_SETTING, DisplayClocks),
    ]
    .into_iter()
    .filter_map(|(bit, reason)| reasons.contains(bit).then_some(reason))
    .collect::<Vec<_>>();
    if unknown {
        result.push(Unknown);
    }
    result
}

fn memory_temperature(device: &Device<'_>) -> Result<Option<f64>, NvmlError> {
    let values = device.field_values_for(&[FieldId(NVML_FI_DEV_MEMORY_TEMP)])?;
    let Some(field) = values.into_iter().next() else {
        return Ok(None);
    };
    let temperature = match field?.value? {
        SampleValue::F64(value) => value,
        SampleValue::U32(value) => f64::from(value),
        SampleValue::U64(value) => value as f64,
        SampleValue::I64(value) => value as f64,
    };
    Ok((-273.15..=255.0)
        .contains(&temperature)
        .then_some(temperature))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_reasons_distinguish_no_limit_and_unknown_flags() {
        assert!(decode_reasons(ThrottleReasons::empty(), false).is_empty());
        assert_eq!(
            decode_reasons(
                ThrottleReasons::GPU_IDLE | ThrottleReasons::SW_POWER_CAP,
                true
            ),
            [
                ClockLimitReason::Idle,
                ClockLimitReason::PowerCap,
                ClockLimitReason::Unknown
            ],
        );
    }
}
