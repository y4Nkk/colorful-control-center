use super::Native;
use crate::{gpu_power::mode::State, powerctl::mode::supply::Review};
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_supply_mode::Trigger;

impl Native<'_> {
    pub(in crate::powerctl::mode::supply) fn freeze(
        &self,
        review: &Review,
        trigger: Trigger,
    ) -> Result<()> {
        let candidate = review.gpu.candidate(
            trigger,
            review.on_ac.ok_or_else(|| anyhow!("供电状态未知"))?,
        );
        if let Some(reason) = &candidate.unavailable_reason {
            bail!("独显模式联动不可执行：{reason}");
        }
        if let Some(action) = candidate.action {
            let current = review
                .gpu
                .current
                .as_ref()
                .ok_or_else(|| anyhow!("独显前态缺失"))?;
            self.links.freeze_node(current, action.target)?;
        }
        Ok(())
    }
    pub(in crate::powerctl::mode::supply) fn authorize(&self) -> Result<()> {
        self.links.authorize()
    }
    pub(in crate::powerctl::mode::supply) fn freeze_restore(
        &self,
        initial: Option<&State>,
        expected: Option<&State>,
    ) -> Result<()> {
        self.links.freeze_restore(initial, expected)
    }
    pub(in crate::powerctl::mode::supply) fn finish(&self) -> Result<()> {
        self.links.finish()
    }
}
