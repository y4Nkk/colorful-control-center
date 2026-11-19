use super::*;
use crate::platform as domain;

pub(super) struct Plan {
    battery: Option<crate::battery::Plan>,
    pd: Option<domain::Plan>,
    usb: Option<domain::usb::Plan>,
    lan: Option<domain::lan::Plan>,
    lan_configured: bool,
}
#[derive(Default)]
pub(super) struct Touched {
    battery: crate::battery::Change,
    pd: bool,
    usb: Vec<domain::usb::Change>,
    lan: bool,
}
impl Plan {
    pub fn prepare(
        hal: &Hal,
        devices: &Scopes<'_>,
        settings: &ProfileSettings,
        mode: Option<&powerctl::mode::Plan>,
    ) -> Result<Self> {
        let active_after_mode = mode
            .and_then(|plan| plan.target.power_plan)
            .map(|id| crate::hal::power_plan::SCHEMES[usize::from(id)]);
        Ok(Self {
            battery: if settings.battery_charge.is_some() || settings.battery_energy.is_some() {
                Some(devices.battery.prepare(
                    hal,
                    settings.battery_charge.as_ref(),
                    settings.battery_energy.as_ref(),
                )?)
            } else {
                None
            },
            pd: settings
                .power_delivery
                .as_ref()
                .map(|target| devices.power_delivery.prepare(hal, target))
                .transpose()?,
            usb: settings
                .usb_policy
                .as_ref()
                .map(|target| devices.usb.prepare(hal, target, active_after_mode))
                .transpose()?,
            lan: settings
                .lan_policy
                .as_ref()
                .map(|target| devices.lan.prepare_profile(hal, target))
                .transpose()?
                .flatten(),
            lan_configured: settings.lan_policy.is_some(),
        })
    }
    pub fn authorize(&self, devices: &mut Scopes<'_>) -> Result<()> {
        devices.lan.authorize_profile(self.lan.as_ref())
    }
    pub fn needs_system(&self) -> bool {
        self.lan.is_some()
    }
    pub fn execute(
        &self,
        hal: &Hal,
        devices: &mut Scopes<'_>,
        touched: &mut Touched,
        steps: &mut Vec<ApplyStep>,
        review: ModeReview<'_>,
    ) -> Result<()> {
        if let Some(plan) = &self.battery {
            guard(steps, review)?;
            stage(
                steps,
                "电池配置",
                devices.battery.execute(hal, plan, &mut touched.battery),
            )?;
        }
        if let Some(plan) = &self.pd {
            guard(steps, review)?;
            stage(
                steps,
                "PD档位",
                devices.power_delivery.execute(hal, plan, &mut touched.pd),
            )?;
        }
        if let Some(plan) = &self.usb {
            guard(steps, review)?;
            stage(
                steps,
                "USB 选择性暂停",
                devices.usb.execute(hal, plan, &mut touched.usb),
            )?;
        }
        if let Some(plan) = &self.lan {
            guard(steps, review)?;
            stage(
                steps,
                "网络驱动策略",
                devices.lan.execute(hal, plan, &mut touched.lan, true),
            )?;
        } else if self.lan_configured {
            stage(steps, "网络驱动策略", Ok(()))?;
        }
        Ok(())
    }
    pub fn rollback(
        &self,
        hal: &Hal,
        devices: &mut Scopes<'_>,
        touched: &mut Touched,
        steps: &mut Vec<ApplyStep>,
    ) {
        if let Some(plan) = self.lan.as_ref().filter(|_| touched.lan) {
            let _ = stage(
                steps,
                "恢复网络驱动策略",
                devices.lan.rollback(hal, plan, &mut touched.lan, true),
            );
        }
        if !touched.usb.is_empty() {
            let _ = stage(
                steps,
                "恢复 USB 选择性暂停",
                devices.usb.rollback(hal, &touched.usb),
            );
        }
        if let Some(plan) = self.pd.as_ref().filter(|_| touched.pd) {
            let _ = stage(
                steps,
                "恢复PD档位",
                devices.power_delivery.rollback(hal, plan),
            );
        }
        if let Some(plan) = self.battery.as_ref().filter(|_| touched.battery.dirty()) {
            let _ = stage(
                steps,
                "恢复电池配置",
                devices.battery.rollback(hal, plan, &mut touched.battery),
            );
        }
    }
}
