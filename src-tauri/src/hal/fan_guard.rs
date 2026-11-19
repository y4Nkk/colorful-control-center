use super::*;

impl Hal {
    pub(crate) fn fan_recovery_pending(&self) -> bool {
        self.fan_recovery_needed.load(Ordering::SeqCst)
    }

    pub fn arm_fan_guard(&self) -> Result<()> {
        self.pulse_fan_guard()?;
        let mut guard = self
            .fan_guard
            .lock()
            .map_err(|_| anyhow!("风扇监护状态异常"))?;
        if self.fan_recovery_needed.load(Ordering::SeqCst) {
            return Err(anyhow!("风扇正在恢复自动模式"));
        }
        if guard.is_some() {
            return Ok(());
        }
        *guard = Some(self.start_fan_guard()?);
        let client = guard.as_mut().ok_or_else(|| anyhow!("风扇监护未启动"))?;
        if let Err(error) = client.arm() {
            client.stop_heartbeats();
            self.fan_recovery_needed.store(true, Ordering::SeqCst);
            return Err(error);
        }
        *self.safety.lock().map_err(|_| anyhow!("监护状态不可用"))? = "监护中".into();
        Ok(())
    }

    fn start_fan_guard(&self) -> Result<crate::watchdog::Watchdog> {
        #[cfg(test)]
        {
            let factory = self
                .test_fan_guard
                .as_ref()
                .ok_or_else(|| anyhow!("隔离测试未注入风扇监护，未启动生产监护进程"))?;
            factory()
        }
        #[cfg(not(test))]
        {
            crate::watchdog::Watchdog::start()
        }
    }

    #[cfg(test)]
    pub(crate) fn fan_guard_armed(&self) -> Result<bool> {
        Ok(self
            .fan_guard
            .lock()
            .map_err(|_| anyhow!("风扇监护状态异常"))?
            .is_some())
    }

    pub fn disarm_fan_guard(&self) -> Result<()> {
        let mut guard = self
            .fan_guard
            .lock()
            .map_err(|_| anyhow!("风扇监护状态异常"))?;
        if guard.is_none() && !self.fan_recovery_needed.load(Ordering::SeqCst) {
            return Ok(());
        }
        let result = guard
            .as_mut()
            .map(|client| client.disarm())
            .unwrap_or(Ok(()));
        self.fan_recovery_needed
            .store(result.is_err(), Ordering::SeqCst);
        if result.is_ok() {
            guard.take();
        }
        *self.safety.lock().map_err(|_| anyhow!("监护状态不可用"))? = match &result {
            Ok(()) => "固件控制".into(),
            Err(error) => format!("风扇监护退出异常：{error:#}"),
        };
        result?;
        if self
            .software_progress
            .lock()
            .map_err(|_| anyhow!("软件曲线进度异常"))?
            .is_none()
        {
            self.record_software_fan_stopped()?;
        }
        Ok(())
    }

    pub fn pulse_fan_guard(&self) -> Result<()> {
        if self.fan_recovery_needed.load(Ordering::SeqCst) {
            return Err(anyhow!("风扇监护正在恢复，暂不接受新的控制"));
        }
        let mut guard = self
            .fan_guard
            .lock()
            .map_err(|_| anyhow!("风扇监护状态异常"))?;
        let mut progress = self
            .software_progress
            .lock()
            .map_err(|_| anyhow!("软件曲线进度异常"))?;
        if progress.is_some_and(|tick| tick.elapsed().as_secs_f64() > 3.0) {
            self.latch_software_fan_fault();
            progress.take();
            // 关闭输入让独立进程恢复；补偿恢复仍由控制锁串行执行。
            if let Some(client) = guard.as_mut() {
                client.stop_heartbeats();
            }
            self.fan_recovery_needed.store(true, Ordering::SeqCst);
            return Err(anyhow!("软件风扇控制循环超过 3 秒未完成，已停止心跳"));
        }
        drop(progress);
        if let Some(client) = guard.as_mut() {
            if let Err(error) = client.pulse() {
                *self.safety.lock().map_err(|_| anyhow!("监护状态不可用"))? = format!("{error:#}");
                if self
                    .software_progress
                    .lock()
                    .map_err(|_| anyhow!("软件曲线进度异常"))?
                    .take()
                    .is_some()
                {
                    self.latch_software_fan_fault();
                }
                client.stop_heartbeats();
                self.fan_recovery_needed.store(true, Ordering::SeqCst);
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn supervise_fan_guard(&self) -> Result<()> {
        let pulse_error = self.pulse_fan_guard().err();
        if !self.fan_recovery_needed.load(Ordering::SeqCst) {
            return Ok(());
        }
        let _write = self.begin_write()?;
        {
            let mut guard = self
                .fan_guard
                .lock()
                .map_err(|_| anyhow!("风扇监护状态异常"))?;
            if let Some(client) = guard.as_mut() {
                client.stop_heartbeats();
                if !client.exited()? {
                    return Err(anyhow!("独立风扇监护仍在恢复，等待其退出后继续"));
                }
            }
            guard.take();
        }
        if crate::fanctl::software::discard(self)? {
            self.latch_software_fan_fault();
        }
        let result = self
            .with_dchu(|d| crate::watchdog::restore_auto(d, self.reviewed_dsdt))
            .and_then(|_| {
                let recorded = self.record_software_fan_stopped();
                self.fan_recovery_needed.store(false, Ordering::SeqCst);
                recorded
            });
        let message = match &result {
            Ok(()) => "监护退出，已恢复自动模式".into(),
            Err(error) if self.fan_recovery_needed.load(Ordering::SeqCst) => {
                format!("自动恢复失败，将重试：{error:#}")
            }
            Err(error) => format!("已恢复风扇自动模式，恢复记录未提交：{error:#}"),
        };
        *self.safety.lock().map_err(|_| anyhow!("监护状态不可用"))? = message.clone();
        if let Some(error) = pulse_error {
            log::error!("[watchdog] {error:#}；{message}");
        }
        result
    }
}
