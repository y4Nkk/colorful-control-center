//! GPU 传感器与当前 NVML 设备的查询能力。
use anyhow::Result;
use nvml_wrapper::{
    Device, Nvml,
    enum_wrappers::device::{Clock, TemperatureSensor},
    error::NvmlError,
};
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(not(test))]
use std::time::{Duration, Instant};

use super::NvidiaSnapshot;
mod details;
pub(crate) use details::GpuDetails;

static CONNECTION_ID: AtomicU32 = AtomicU32::new(1);

pub struct Nvidia {
    nvml: Nvml,
    device: Connection,
}

#[derive(Default)]
pub(super) struct Provider {
    current: Option<Nvidia>,
    #[cfg(not(test))]
    retry: Option<(Instant, String)>,
}

impl Provider {
    pub(super) fn query<T>(
        &mut self,
        #[cfg_attr(test, allow(unused_variables))] initialize: bool,
        sample: impl FnOnce(&mut Nvidia) -> Result<T>,
    ) -> Result<Option<T>> {
        #[cfg(not(test))]
        if initialize && self.current.is_none() {
            self.connect()?;
        }
        let result = self.current.as_mut().map(sample).transpose();
        if result.is_err() {
            self.current = None;
        }
        result
    }

    #[cfg(not(test))]
    fn connect(&mut self) -> Result<()> {
        if let Some((retry_at, error)) = &self.retry
            && Instant::now() < *retry_at
        {
            anyhow::bail!("{error}");
        }
        self.retry = None;
        match Nvidia::init() {
            Ok(nvidia) => {
                self.current = Some(nvidia);
                Ok(())
            }
            Err(error) => {
                self.retry = Some((
                    Instant::now() + Duration::from_secs(5),
                    format!("{error:#}"),
                ));
                Err(error)
            }
        }
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.current.is_none()
    }
}

#[derive(Clone, Copy, Debug)]
enum Query {
    Name,
    Temperature,
    Utilization,
    Memory,
    GraphicsClock,
    MemoryClock,
    PowerDraw,
    PowerLimit,
    CudaCores,
    BusWidth,
    Encoder,
    Decoder,
    Performance,
    PcieGen,
    PcieWidth,
    PcieMaxGen,
    PcieMaxWidth,
    ClockLimits,
    MemoryTemperature,
    FanTargetRpm,
}

#[derive(Default)]
struct QuerySupport(u32);

impl QuerySupport {
    fn read<T>(
        &mut self,
        field: Query,
        query: impl FnOnce() -> Result<T, NvmlError>,
    ) -> Result<Option<T>, NvmlError> {
        let mask = 1 << field as u32;
        if self.0 & mask != 0 {
            return Ok(None);
        }
        match query() {
            Ok(value) => Ok(Some(value)),
            Err(
                NvmlError::NotSupported
                | NvmlError::FunctionNotFound
                | NvmlError::FailedToLoadSymbol(_),
            ) => {
                self.0 |= mask;
                Ok(None)
            }
            Err(error @ (NvmlError::GpuLost | NvmlError::Uninitialized)) => Err(error),
            Err(error) => {
                log::debug!("[nvml] {field:?} 读取失败：{error}");
                Ok(None)
            }
        }
    }

    fn static_value<T: Clone>(
        &mut self,
        slot: &mut Option<T>,
        field: Query,
        query: impl FnOnce() -> Result<T, NvmlError>,
    ) -> Result<Option<T>, NvmlError> {
        if slot.is_none() {
            *slot = self.read(field, query)?;
        }
        Ok(slot.clone())
    }
}

#[derive(Default)]
struct Connection {
    handle: Option<usize>,
    id: u32,
    support: QuerySupport,
    name: Option<String>,
    cuda_cores: Option<u32>,
    memory_bus_width: Option<u32>,
    max_link_gen: Option<u32>,
    max_link_width: Option<u32>,
}

impl Connection {
    fn bind(&mut self, handle: usize) {
        if self.handle != Some(handle) {
            *self = Self {
                handle: Some(handle),
                id: CONNECTION_ID.fetch_add(1, Ordering::Relaxed),
                ..Default::default()
            };
        }
    }

    fn temperature(&mut self, device: &Device<'_>) -> Result<Option<f64>, NvmlError> {
        self.support
            .read(Query::Temperature, || {
                device.temperature(TemperatureSensor::Gpu)
            })
            .map(|value| value.map(f64::from))
    }

    fn snapshot(&mut self, device: &Device<'_>) -> Result<NvidiaSnapshot> {
        let temp_c = self.temperature(device)?;
        let support = &mut self.support;
        let name = support.static_value(&mut self.name, Query::Name, || device.name())?;
        let cuda_cores = support.static_value(&mut self.cuda_cores, Query::CudaCores, || {
            device.num_cores()
        })?;
        let memory_bus_width_bits =
            support.static_value(&mut self.memory_bus_width, Query::BusWidth, || {
                device.memory_bus_width()
            })?;
        let memory = support.read(Query::Memory, || device.memory_info())?;
        let usage = support.read(Query::Utilization, || device.utilization_rates())?;
        const MIB: f64 = 1024.0 * 1024.0;
        Ok(NvidiaSnapshot {
            connection_id: self.id,
            name: name.unwrap_or_default(),
            temp_c,
            utilization_pct: usage.map(|value| f64::from(value.gpu)),
            memory_used_pct: memory
                .as_ref()
                .filter(|value| value.total > 0 && value.used <= value.total)
                .map(|value| value.used as f64 * 100.0 / value.total as f64),
            mem_total_mb: memory
                .as_ref()
                .filter(|value| value.total > 0)
                .map(|value| value.total as f64 / MIB),
            graphics_clock_mhz: support
                .read(Query::GraphicsClock, || device.clock_info(Clock::Graphics))?,
            memory_clock_mhz: support
                .read(Query::MemoryClock, || device.clock_info(Clock::Memory))?,
            power_draw_w: support
                .read(Query::PowerDraw, || device.power_usage())?
                .map(|value| f64::from(value) / 1000.0),
            power_limit_w: support
                .read(Query::PowerLimit, || device.power_management_limit())?
                .map(|value| f64::from(value) / 1000.0),
            cuda_cores: cuda_cores.filter(|value| *value > 0),
            memory_bus_width_bits: memory_bus_width_bits.filter(|value| *value > 0),
        })
    }
}

impl Nvidia {
    pub fn init() -> Result<Self> {
        Ok(Self {
            nvml: Nvml::init()?,
            device: Connection::default(),
        })
    }

    fn query<T>(
        &mut self,
        sample: impl FnOnce(&mut Connection, &Device<'_>) -> Result<T>,
    ) -> Result<T> {
        let device = self.nvml.device_by_index(0)?;
        // 句柄只用于比较设备身份，所有查询仍使用带生命周期的包装。
        self.device.bind(unsafe { device.handle().addr() });
        sample(&mut self.device, &device)
    }

    pub(super) fn sample(&mut self) -> Result<NvidiaSnapshot> {
        self.query(Connection::snapshot)
    }

    pub(crate) fn sample_temperature(&mut self) -> Result<Option<f64>> {
        self.query(|connection, device| Ok(connection.temperature(device)?))
    }

    pub(super) fn sample_details(&mut self, expected_id: u32) -> Result<Option<GpuDetails>> {
        if self.device.id != expected_id {
            return Ok(None);
        }
        self.query(|connection, device| {
            (connection.id == expected_id)
                .then(|| connection.details(device))
                .transpose()
        })
    }
}

#[cfg(test)]
mod tests;
