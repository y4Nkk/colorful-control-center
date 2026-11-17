use super::{
    codec,
    codec_store::Store,
    engine::{self, Binding},
};
use crate::{
    hal::{Hal, oem_dchu::Dchu},
    hotkeys::screen_brightness,
};
use anyhow::{Context, Result, ensure};

pub(super) struct Native<'a> {
    pub hal: &'a Hal,
    pub screen: &'a screen_brightness::Controller,
    pub store: &'a Store,
}
pub(super) fn pc_beep(io: &Dchu) -> Result<bool> {
    let (valid, value) = io.read_int(70);
    ensure!(valid, "原厂WMI70能力读取无效或为回声");
    Ok(value as u32 & (1 << 13) != 0)
}
pub(super) fn low_bits(io: &Dchu) -> Result<bool> {
    let mut request = [0; 256];
    request[..7].copy_from_slice(&[1, 5, 0x51, 1, 0, 0, 177]);
    Ok(io.exchange_package(4, &request)?[1] & 0x30 != 0)
}
impl Native<'_> {
    pub fn codec(&self, enabled: bool) -> codec::Profile {
        if !enabled && self.store.load().is_ok_and(|record| record.is_none()) {
            return codec::Profile {
                current: None,
                initial: None,
                error: None,
                restore_revision: None,
                restore_error: None,
            };
        }
        #[cfg(not(test))]
        {
            codec::preview(&codec::Native, self.store)
        }
        #[cfg(test)]
        {
            codec::Profile {
                current: None,
                initial: None,
                error: Some("隔离测试未注入PC Beep传输".into()),
                restore_revision: None,
                restore_error: None,
            }
        }
    }
    pub fn inspect(
        &self,
    ) -> (
        Result<Binding>,
        Result<screen_brightness::LowBatteryProfile>,
        codec::Profile,
    ) {
        let enabled = self.hal.with_dchu(pc_beep);
        let brightness = self.screen.read_alert(self.hal);
        let audio = self.codec(enabled.as_ref().is_ok_and(|enabled| *enabled));
        let binding = (|| {
            let enabled = *enabled
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error:#}"))?;
            let current = brightness
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error:#}"))?;
            let snapshot = current.current.as_ref().context("屏幕亮度来源不可用")?;
            let identity = current.identity.clone().with_context(|| {
                current
                    .error
                    .clone()
                    .unwrap_or_else(|| "屏幕亮度事务未完成".into())
            })?;
            ensure!(
                audio.restore_error.is_none(),
                "{}",
                audio.restore_error.as_deref().unwrap_or("")
            );
            ensure!(
                enabled || audio.initial.is_none(),
                "PC Beep能力已变化，请先恢复初始控件"
            );
            let codec = if enabled {
                let current = audio.current.as_ref().with_context(|| {
                    audio
                        .error
                        .clone()
                        .unwrap_or_else(|| "PC Beep控件不可用".into())
                })?;
                Some(super::super::model::digest(&(
                    &current.endpoint,
                    &current.volume_part,
                    &current.mute_part,
                ))?)
            } else {
                None
            };
            Ok(Binding {
                brightness: identity,
                customer: snapshot.rule.customer,
                low_control: snapshot.rule.low_supported(),
                pc_beep: enabled,
                codec,
            })
        })();
        (binding, brightness, audio)
    }
    pub fn restore_codec(&self, expected: &str) -> Result<()> {
        let _gate = self.hal.begin_write()?;
        engine::Io::available(self)?;
        #[cfg(not(test))]
        {
            codec::restore(&codec::Native, self.store, expected, &mut || Ok(()))
        }
        #[cfg(test)]
        {
            let _ = expected;
            anyhow::bail!("隔离测试未注入PC Beep传输")
        }
    }
    pub fn cleanup_codec(&self) -> Result<()> {
        if self.store.load()?.is_none() {
            return Ok(());
        }
        let profile = self.codec(true);
        self.restore_codec(profile.restore_revision.as_deref().with_context(|| {
            profile
                .restore_error
                .or(profile.error)
                .unwrap_or_else(|| "PC Beep恢复不可用".into())
        })?)
    }
}
impl engine::Io for Native<'_> {
    fn binding(&self) -> Result<Binding> {
        self.inspect().0
    }
    fn available(&self) -> Result<()> {
        self.hal.ensure_mode_change_allowed()?;
        #[cfg(not(test))]
        {
            crate::hal::oem_owner::ensure_owner_absent()?;
            crate::battery::calibration::ownership::ensure_available()
        }
        #[cfg(test)]
        {
            anyhow::bail!("隔离测试未注入控制者检查")
        }
    }
    fn brightness(
        &self,
        binding: &Binding,
        value: u8,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        self.screen
            .alert_absolute(self.hal, &binding.brightness, value, permit)
    }
    fn low_setting(
        &self,
        binding: &Binding,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        self.screen.alert_low(self.hal, &binding.brightness, permit)
    }
    fn mute(
        &self,
        binding: &Binding,
        value: bool,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        let _gate = self.hal.begin_write()?;
        self.available()?;
        ensure!(
            self.hal.with_dchu(pc_beep)? == binding.pc_beep,
            "原厂PC Beep能力已变化"
        );
        #[cfg(not(test))]
        {
            use codec::Io;
            let current = codec::Native.capture()?;
            ensure!(
                Some(super::super::model::digest(&(
                    &current.endpoint,
                    &current.volume_part,
                    &current.mute_part
                ))?) == binding.codec,
                "PC Beep控件身份已变化"
            );
            codec::change(&codec::Native, self.store, &current, value, permit)?;
            Ok(())
        }
        #[cfg(test)]
        {
            let _ = (value, permit);
            anyhow::bail!("隔离测试未注入PC Beep传输")
        }
    }
    fn play(&self, enabled: bool) -> Result<()> {
        #[cfg(not(test))]
        {
            crate::hal::beep_audio::silence(enabled)
        }
        #[cfg(test)]
        {
            let _ = enabled;
            anyhow::bail!("隔离测试未注入静音循环")
        }
    }
    fn low_bits(&self) -> Result<bool> {
        let _gate = self.hal.begin_write()?;
        self.hal.with_dchu(low_bits)
    }
    fn notify(&self) -> Result<()> {
        let _gate = self.hal.begin_write()?;
        self.available()?;
        self.hal.with_dchu(|io| io.set_data_sub(121, 44, 1))
    }
}
