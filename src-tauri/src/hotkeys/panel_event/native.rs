use super::*;
pub(super) struct Native<'a>(pub &'a Hal);
impl Io for Native<'_> {
    fn current(&self) -> Result<Source> {
        if !self.0.reviewed_dsdt {
            bail!("当前机型的面板事件协议尚未核对");
        }
        let panels = self.0.panel_inventory()?;
        if !panels.unresolved.is_empty() {
            bail!("面板清单不完整：{}", panels.unresolved.join("；"));
        }
        let oled = panels.monitors.iter().any(|m| m.oled);
        let candidate = panels
            .monitors
            .iter()
            .any(|m| NVSR_SIGNATURES.contains(&m.signature.as_str()));
        let nvsr_uid = if candidate {
            let value = self.0.with_dchu(|d| {
                let (ok, value) = d.read_int(84);
                if !ok || value < 0 {
                    bail!("原厂NVSR条件读取失败");
                }
                Ok(value)
            })?;
            if value != 1 {
                Some(crate::hal::panel_aux::identify()?)
            } else {
                None
            }
        } else {
            None
        };
        if !oled && nvsr_uid.is_none() {
            bail!("当前面板无需原厂OLED唤醒或NVSR修复");
        }
        Ok(Source {
            panels,
            nvsr_uid,
            oled,
        })
    }
    fn nvsr(&self, uid: u32) -> Result<()> {
        let _gate = self.0.begin_write()?;
        crate::hal::panel_aux::repair(uid)
    }
    fn oled(&self) -> Result<()> {
        let _gate = self.0.begin_write()?;
        self.0.with_dchu(|d| d.set_u32(94, 1))
    }
    fn ensure(&self) -> Result<()> {
        self.0.ensure_mode_change_allowed()?;
        crate::hal::oem_owner::ensure_owner_absent()
    }
}
