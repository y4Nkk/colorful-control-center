use crate::{
    battery,
    fanctl::dust,
    keyboard, keymap,
    lighting::{self, lightbar, options, perkey},
    platform,
};
use crate::{gpuctl::GpuController, hal::Hal, powerctl::mode::links};
use anyhow::Result;
use tauri::{AppHandle, Manager};

#[derive(Clone, Copy)]
pub(crate) struct Devices<'a> {
    #[cfg(test)]
    pub mode_source: Option<&'a dyn links::Source>,
    pub light_bar: &'a lightbar::Controller,
    pub multi_zone: &'a lighting::multizone::Controller,
    pub dust: &'a dust::Controller,
    pub keyboard: &'a keyboard::Controller,
    pub microphones: &'a crate::microphone::Controller,
    pub touchpad: &'a crate::touchpad::Controller,
    pub lighting: &'a lighting::LightingController,
    pub lighting_options: &'a options::Controller,
    pub perkey: &'a perkey::Controller,
    pub keymap: &'a keymap::Controller,
    pub power_delivery: &'a platform::Controller,
    pub usb: &'a platform::usb::Controller,
    pub battery: &'a battery::Controller,
    pub lan: &'a platform::lan::Controller,
    pub gpu_power: &'a crate::gpu_power::Controller,
    pub graphics_mode: &'a crate::graphics_mode::Controller,
    pub display: &'a crate::display::Controller,
    pub display_owner: Option<u64>,
}
pub(super) struct Scopes<'a> {
    pub dust: dust::Scope<'a>,
    pub light_bar: lightbar::Scope<'a>,
    pub multi_zone: lighting::multizone::Scope<'a>,
    pub keyboard: keyboard::Scope<'a>,
    pub microphones: crate::microphone::Scope<'a>,
    pub touchpad: crate::touchpad::Scope<'a>,
    pub lighting: lighting::Scope<'a>,
    pub lighting_options: options::Scope<'a>,
    pub perkey: perkey::Scope<'a>,
    pub keymap: keymap::Scope<'a>,
    pub power_delivery: platform::Scope<'a>,
    pub usb: platform::usb::Scope<'a>,
    pub battery: battery::Scope<'a>,
    pub lan: platform::lan::Scope<'a>,
    pub mode: links::Session<'a>,
    pub graphics_mode: crate::graphics_mode::Scope<'a>,
    pub display: &'a crate::display::Controller,
    pub display_owner: Option<u64>,
}
impl<'a> Devices<'a> {
    pub(crate) fn from_app(app: &'a AppHandle) -> Self {
        Self {
            #[cfg(test)]
            mode_source: None,
            light_bar: app.state::<lightbar::Controller>().inner(),
            multi_zone: app.state::<lighting::multizone::Controller>().inner(),
            dust: app.state::<dust::Controller>().inner(),
            keyboard: app.state::<keyboard::Controller>().inner(),
            microphones: app.state::<crate::microphone::Controller>().inner(),
            touchpad: app.state::<crate::touchpad::Controller>().inner(),
            lighting: app.state::<lighting::LightingController>().inner(),
            lighting_options: app.state::<options::Controller>().inner(),
            perkey: app.state::<perkey::Controller>().inner(),
            keymap: app.state::<keymap::Controller>().inner(),
            power_delivery: app.state::<platform::Controller>().inner(),
            usb: app.state::<platform::usb::Controller>().inner(),
            battery: app.state::<battery::Controller>().inner(),
            lan: app.state::<platform::lan::Controller>().inner(),
            gpu_power: app.state::<crate::gpu_power::Controller>().inner(),
            graphics_mode: app.state::<crate::graphics_mode::Controller>().inner(),
            display: app.state::<crate::display::Controller>().inner(),
            display_owner: None,
        }
    }
    pub(super) fn mode_context(
        &self,
        hal: &'a Hal,
        gpu: &'a GpuController,
    ) -> Result<links::Context<'a>> {
        #[cfg(not(test))]
        {
            Ok(links::Context::native(hal, gpu, self.gpu_power))
        }
        #[cfg(test)]
        {
            let source = self
                .mode_source
                .ok_or_else(|| anyhow::anyhow!("隔离档案未注入模式来源，禁止访问真实供电或固件"))?;
            Ok(links::Context::with_source(
                hal,
                gpu,
                self.gpu_power,
                Box::new(source),
            ))
        }
    }
    pub(super) fn scopes(&self, hal: &'a Hal, gpu: &'a GpuController) -> Result<Scopes<'a>> {
        Ok(Scopes {
            dust: self.dust.scope()?,
            light_bar: self.light_bar.scope()?,
            multi_zone: self.multi_zone.scope()?,
            keyboard: self.keyboard.scope()?,
            microphones: self.microphones.scope()?,
            touchpad: self.touchpad.scope()?,
            lighting: self.lighting.scope()?,
            lighting_options: self.lighting_options.scope()?,
            perkey: self.perkey.scope()?,
            keymap: self.keymap.scope()?,
            power_delivery: self.power_delivery.scope()?,
            usb: self.usb.scope()?,
            battery: self.battery.scope()?,
            lan: self.lan.scope()?,
            mode: self.mode_context(hal, gpu)?.session()?,
            graphics_mode: self.graphics_mode.scope()?,
            display: self.display,
            display_owner: self.display_owner,
        })
    }
}
