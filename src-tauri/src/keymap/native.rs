use super::*;
use crate::hal::oem_owner::ensure_owner_absent;
use ccc_core::settings::{self, Patch};
pub(super) struct Native<'a> {
    pub hal: &'a Hal,
    pub controller: &'a Controller,
}
impl Io for Native<'_> {
    fn facts(&self) -> Result<Facts> {
        let capability: [u8; 15] = self
            .hal
            .with_dchu(|io| io.read_bios_capabilities(0, 15))?
            .try_into()
            .map_err(|_| anyhow!("宏能力页长度无效"))?;
        if !supported(&capability)? {
            return Ok(Facts {
                capability,
                flags: [0, 0],
                original: None,
                original_error: None,
            });
        }
        let flags = self
            .hal
            .with_dchu(|io| io.read_settings(1, 8, 2))?
            .try_into()
            .map_err(|_| anyhow!("宏启用字段长度无效"))?;
        let (original, original_error) =
            match original::read(std::path::Path::new(r"C:\ComboKey\profile.xml")) {
                Ok(source) => (source, None),
                Err(error) => (None, Some(format!("{error:#}"))),
            };
        Ok(Facts {
            capability,
            flags,
            original,
            original_error,
        })
    }
    fn ensure_available(&self) -> Result<()> {
        if !std::path::Path::new(crate::hal::key_filter::DLL_PATH).is_file() {
            bail!("未找到已核对的原厂HKFltrLib过滤库");
        }
        ensure_owner_absent()
    }
    fn replace(&self, target: &Configuration) -> Result<()> {
        crate::hal::key_filter::Library::load()?.replace(target)
    }
    fn save_flags(&self, flags: [u8; 2], scope: &[Device]) -> Result<()> {
        self.hal.with_dchu(|io| {
            let patches: Vec<_> = scope
                .iter()
                .map(|device| Patch::new(8 + model::index(*device), [flags[model::index(*device)]]))
                .collect();
            settings::apply(io, 1, &patches, None).map_err(|e| anyhow!(e))?;
            let current = io.read_settings(1, 8, 2)?;
            if scope
                .iter()
                .any(|device| current[model::index(*device)] != flags[model::index(*device)])
            {
                bail!("宏启用保存回读不一致");
            }
            Ok(())
        })
    }
    fn prepare_programs(
        &self,
        record: &model::Record,
        gate: Arc<Mutex<()>>,
    ) -> Result<Option<launch::Session>> {
        launch::prepare(
            self.controller.path.clone(),
            record.clone(),
            self.controller.recording.clone(),
            gate,
        )
    }
}
