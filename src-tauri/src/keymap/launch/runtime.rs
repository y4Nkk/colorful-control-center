use super::*;
use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};

#[derive(Default)]
pub(super) struct Control {
    pub stop: AtomicBool,
    pub active: AtomicBool,
    pub epoch: AtomicU64,
    pub state: Mutex<Status>,
}
impl Control {
    pub fn accepts(&self, epoch: u64) -> bool {
        !self.stop.load(Ordering::SeqCst)
            && self.active.load(Ordering::SeqCst)
            && self.epoch.load(Ordering::SeqCst) == epoch
    }
    pub fn pause(&self) {
        self.active.store(false, Ordering::SeqCst);
        self.epoch.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut status) = self.state.lock() {
            status.running = false;
        }
    }
    pub fn fail(&self, error: anyhow::Error) {
        self.stop.store(true, Ordering::SeqCst);
        self.pause();
        if let Ok(mut status) = self.state.lock() {
            status.error = Some(format!("{error:#}"));
        }
    }
}
pub(super) trait Input {
    fn poll(&mut self) -> Result<Vec<event_log::Record>>;
    fn discard_pending(&mut self) -> Result<()>;
}
pub(super) fn run(
    input: &mut impl Input,
    control: &Control,
    gate: &Mutex<()>,
    mut execute: impl FnMut(&event_log::Record) -> Result<()>,
) -> Result<()> {
    let mut ready_epoch = None;
    while !control.stop.load(Ordering::SeqCst) {
        let epoch = control.epoch.load(Ordering::SeqCst);
        if control.accepts(epoch) {
            match gate.try_lock() {
                Ok(_guard) if control.accepts(epoch) => {
                    if ready_epoch != Some(epoch) {
                        // 提交后的首次消费丢弃准备/暂停期间的按键，不能把积压事件当作新授权。
                        input.discard_pending()?;
                        ready_epoch = Some(epoch);
                        if control.accepts(epoch) {
                            control
                                .state
                                .lock()
                                .map_err(|_| anyhow!("程序监听状态异常"))?
                                .running = true;
                        }
                    } else {
                        for event in input.poll()? {
                            if !control.accepts(epoch) {
                                break;
                            }
                            execute(&event)?;
                        }
                    }
                }
                Ok(_) | Err(std::sync::TryLockError::WouldBlock) => (),
                Err(std::sync::TryLockError::Poisoned(_)) => bail!("控制事务异常，程序监听已停止"),
            }
        }
        thread::park_timeout(Duration::from_millis(20));
    }
    Ok(())
}
