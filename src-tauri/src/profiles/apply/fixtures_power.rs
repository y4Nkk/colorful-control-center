use super::*;

impl crate::hal::power_plan::PowerAccess for Device {
    fn lid_action(&self, _: u128) -> anyhow::Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入合盖计划动作")
    }
    fn battery_action(
        &self,
        _: u128,
        _: crate::hal::power_plan::BatteryAction,
    ) -> anyhow::Result<u32> {
        anyhow::bail!("此隔离提供者未注入电池计划动作")
    }
    fn battery_level(
        &self,
        _: u128,
        _: crate::hal::power_plan::BatteryLevel,
    ) -> anyhow::Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入电池计划阈值")
    }
    fn set_battery_level(
        &self,
        _: u128,
        _: crate::hal::power_plan::BatteryLevel,
        _: bool,
        _: u32,
    ) -> anyhow::Result<()> {
        anyhow::bail!("此隔离提供者未注入电池计划阈值")
    }
    fn brightness(&self, _: u128) -> anyhow::Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入屏幕亮度字段")
    }
    fn set_brightness(&self, _: u128, _: bool, _: u32) -> anyhow::Result<()> {
        anyhow::bail!("此隔离提供者未注入屏幕亮度字段")
    }

    fn schemes(&self) -> anyhow::Result<Vec<u128>> {
        Ok(self.0.lock().unwrap().plan_values.keys().copied().collect())
    }
    fn scheme_name(&self, _: u128) -> anyhow::Result<String> {
        anyhow::bail!("档案测试未注入计划名称")
    }
    fn check_active_scheme_write(&self) -> anyhow::Result<()> {
        anyhow::bail!("档案测试未注入计划切换权限")
    }
    fn usb(&self, scheme: u128) -> anyhow::Result<[u32; 2]> {
        self.0
            .lock()
            .unwrap()
            .usb_values
            .get(&scheme)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("隔离USB计划不存在"))
    }
    fn set_usb(&self, scheme: u128, ac: bool, value: u32) -> anyhow::Result<()> {
        let mut state = self.0.lock().unwrap();
        state.events.push(format!("USB:{scheme:x}:{ac}:{value}"));
        state.usb_values.get_mut(&scheme).unwrap()[usize::from(!ac)] = value;
        state.usb_writes += 1;
        if state.fail_usb_at.contains(&state.usb_writes) {
            anyhow::bail!("隔离USB部分写入后失败");
        }
        Ok(())
    }
    fn active(&self) -> anyhow::Result<u128> {
        Ok(self.0.lock().unwrap().active_plan)
    }
    fn aspm(&self, scheme: u128) -> anyhow::Result<[u32; 2]> {
        self.0
            .lock()
            .unwrap()
            .plan_values
            .get(&scheme)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("计划不存在"))
    }
    fn set_aspm(&self, scheme: u128, ac: bool, value: u32) -> anyhow::Result<()> {
        let mut state = self.0.lock().unwrap();
        state.events.push(format!("ASPM:{ac}:{value}"));
        state.plan_values.get_mut(&scheme).unwrap()[usize::from(!ac)] = value;
        Ok(())
    }
    fn activate(&self, scheme: u128) -> anyhow::Result<()> {
        let mut state = self.0.lock().unwrap();
        state.events.push(format!("PLAN:{scheme:x}"));
        state.active_plan = scheme;
        if state.fail_plan && scheme == crate::hal::power_plan::SCHEMES[2] {
            anyhow::bail!("计划部分激活后失败");
        }
        Ok(())
    }
}
