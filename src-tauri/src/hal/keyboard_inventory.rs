use anyhow::Result;

#[cfg(not(test))]
mod native;

impl super::Hal {
    pub(crate) fn usb_keyboard_present(&self) -> Result<bool> {
        #[cfg(test)]
        {
            let query = self
                .test_usb_keyboard
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("隔离USB键盘清单未配置，禁止回退系统查询"))?;
            query()
        }
        #[cfg(not(test))]
        {
            native::usb_present()
        }
    }
}
