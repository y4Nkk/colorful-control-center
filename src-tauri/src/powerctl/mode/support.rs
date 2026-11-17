use anyhow::{Result, anyhow, bail};
use ccc_core::settings::SettingsAccess;

pub(super) fn issue(io: &impl SettingsAccess) -> Result<Option<String>> {
    let raw = io.read(0, 2, 4).map_err(|error| anyhow!(error))?;
    let product = ccc_core::oem_identity::product_id(
        raw.try_into()
            .map_err(|_| anyhow!("原厂产品标识长度无效"))?,
    );
    Ok(
        matches!(product, 20720 | 20724)
            .then(|| format!("尚未支持{product}机型的完整电源模式控制")),
    )
}

pub(super) fn ensure(io: &impl SettingsAccess) -> Result<()> {
    if let Some(reason) = issue(io)? {
        bail!(reason);
    }
    Ok(())
}
