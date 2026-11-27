use super::{Access, Baseline, Event, Facts, events::Log, power_plan};
use crate::{
    battery::{Io, energy, native::Native as Battery},
    hal::Hal,
};
use anyhow::{Context, Result, bail};
use windows_sys::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
        System::{Power::*, Services::*, Threading::*},
    },
    core::w,
};

pub(super) struct Native<'a> {
    pub hal: &'a Hal,
    pub energy_path: std::path::PathBuf,
    log: Option<Log>,
    awake: Option<HANDLE>,
    owned_plan: Option<String>,
}
impl<'a> Native<'a> {
    pub fn new(hal: &'a Hal, energy_path: std::path::PathBuf) -> Result<Self> {
        Ok(Self {
            hal,
            energy_path,
            log: Some(Log::open()?),
            awake: None,
            owned_plan: None,
        })
    }
    pub fn task(hal: &'a Hal, energy_path: std::path::PathBuf, operation: &str) -> Result<Self> {
        let mut this = Self::new(hal, energy_path)?;
        this.owned_plan = Some(crate::hal::power_plan::format_guid(power_plan::owned(
            operation,
        )?));
        Ok(this)
    }
    pub fn recovery(
        hal: &'a Hal,
        energy_path: std::path::PathBuf,
        operation: &str,
    ) -> Result<Self> {
        Ok(Self {
            hal,
            energy_path,
            log: None,
            awake: None,
            owned_plan: Some(crate::hal::power_plan::format_guid(power_plan::owned(
                operation,
            )?)),
        })
    }
    fn plan(&self) -> Result<(power_plan::native::Native, &str)> {
        Ok((
            power_plan::native::Native {
                resource_path: self.energy_path.with_file_name("battery-calibration.pow"),
            },
            self.owned_plan
                .as_deref()
                .context("校准电源计划尚未绑定本次任务")?,
        ))
    }
}
impl Drop for Native<'_> {
    fn drop(&mut self) {
        if let Some(handle) = self.awake.take() {
            unsafe {
                CloseHandle(handle);
            }
        }
    }
}
fn service_running() -> Result<()> {
    unsafe {
        let manager = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if manager.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        let service = OpenServiceW(manager, w!("DCHUService"), SERVICE_QUERY_STATUS);
        let opened_error = std::io::Error::last_os_error();
        CloseServiceHandle(manager);
        if service.is_null() {
            return Err(opened_error.into());
        }
        let mut status = SERVICE_STATUS::default();
        let ok = QueryServiceStatus(service, &mut status);
        let query_error = std::io::Error::last_os_error();
        CloseServiceHandle(service);
        if ok == 0 {
            return Err(query_error.into());
        }
        if status.dwCurrentState != SERVICE_RUNNING {
            bail!("原厂通知服务未运行，未接管校准");
        }
    }
    Ok(())
}
impl Access for Native<'_> {
    fn verify_baseline(&mut self, baseline: &Baseline) -> Result<()> {
        if energy::capture_calibration(self.hal, &self.energy_path)? != *baseline {
            bail!("充电或节能策略已变化，请重新预览校准");
        }
        Ok(())
    }
    fn ready(&mut self) -> Result<()> {
        power_plan::bundle()?;
        if self.log.is_none() {
            bail!("恢复通道不能接管校准任务");
        }
        crate::hal::oem_owner::ensure_battery_owner_absent()?;
        service_running()?;
        super::events::ensure_logs()?;
        self.hal.with_dchu(|io| {
            if io.read_settings(1, 12, 1)? != [1] {
                bail!("原厂服务未声明ACPI通知通道就绪");
            }
            Ok(())
        })
    }
    fn facts(&mut self) -> Result<Facts> {
        let battery = Battery { hal: self.hal };
        let power = battery.power()?;
        let diag = battery
            .diagnostics()?
            .context("本机未声明原厂电池校准能力")?;
        Ok(Facts {
            present: power.present,
            ac_online: power.ac_online,
            percent: power.percent,
            battery_status: diag.battery_status,
            permanent_failure: diag.permanent_failure_status,
            stop_threshold: diag.stop_charging_threshold,
        })
    }
    fn events(&mut self) -> Result<Vec<Event>> {
        self.log.as_mut().context("校准通知尚未订阅")?.poll()
    }
    fn send(&mut self, command: i32, value: u32) -> Result<()> {
        self.hal.with_dchu(|io| io.set_u32(command, value))
    }
    fn keep_awake(&mut self, enabled: bool) -> Result<()> {
        if !enabled {
            if let Some(handle) = self.awake {
                if unsafe { PowerClearRequest(handle, PowerRequestSystemRequired) } == 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                unsafe {
                    CloseHandle(handle);
                }
                self.awake = None;
            }
            return Ok(());
        }
        if self.awake.is_some() {
            return Ok(());
        }
        let mut reason: Vec<u16> = "Colorful Control Center 电池校准"
            .encode_utf16()
            .chain([0])
            .collect();
        let context = REASON_CONTEXT {
            Version: 0,
            Flags: 1,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: reason.as_mut_ptr(),
            },
        };
        let handle = unsafe { PowerCreateRequest(&context) };
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        if unsafe { PowerSetRequest(handle, PowerRequestSystemRequired) } == 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                CloseHandle(handle);
            }
            return Err(error.into());
        }
        self.awake = Some(handle);
        Ok(())
    }
    fn restore_policy(&mut self, baseline: &Baseline) -> Result<()> {
        energy::restore_calibration(self.hal, baseline)
    }
    fn prepare_power_plan(&mut self, before: &str) -> Result<()> {
        let (io, target) = self.plan()?;
        power_plan::prepare(&io, before, target)
    }
    fn enter_power_plan(&mut self, before: &str) -> Result<()> {
        let (io, target) = self.plan()?;
        power_plan::enter(&io, before, target)
    }
    fn check_power_plan(&mut self) -> Result<()> {
        let (io, target) = self.plan()?;
        power_plan::check(&io, target)
    }
    fn restore_power_plan(&mut self, before: &str) -> Result<()> {
        let (io, target) = self.plan()?;
        power_plan::restore(&io, before, target)
    }
}
