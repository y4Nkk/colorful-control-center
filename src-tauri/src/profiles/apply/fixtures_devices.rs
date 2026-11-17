use super::Fixture;

impl Fixture {
    pub fn devices(&self) -> crate::profiles::Devices<'_> {
        crate::profiles::Devices {
            mode_source: Some(self.device.as_ref()),
            light_bar: &self.light_bar,
            multi_zone: &self.multi_zone,
            dust: &self.dust,
            keyboard: &self.keyboard,
            microphones: &self.microphones,
            touchpad: &self.touchpad,
            lighting: &self.lighting,
            lighting_options: &self.lighting_options,
            perkey: &self.perkey,
            keymap: &self.keymap,
            power_delivery: &self.power_delivery,
            usb: &self.usb,
            battery: &self.battery.controller,
            lan: &self.lan.controller,
            display: &self.display.controller,
            display_owner: None,
            gpu_power: &self.gpu_power,
            graphics_mode: &self.graphics_mode.controller,
        }
    }
}
