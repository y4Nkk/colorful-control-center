use anyhow::Result;
use ccc_core::panel_hotkey::Activity;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub(super) enum Event {
    Pending,
    Exited(u32),
    Input(Activity),
}
pub(super) trait Resource: Send {
    fn start(&mut self) -> Result<()>;
    fn poll(&mut self) -> Result<Event>;
    fn stop(&mut self) -> Result<()>;
}

// 首个活动只入队类别；准备期间、后续活动和停止后的活动均不保留。
#[derive(Clone)]
pub(super) struct Input {
    active: Arc<AtomicBool>,
    taken: Arc<AtomicBool>,
    sender: mpsc::SyncSender<Activity>,
}
impl Input {
    pub fn channel() -> (Self, mpsc::Receiver<Activity>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        (
            Self {
                active: Arc::default(),
                taken: Arc::default(),
                sender,
            },
            receiver,
        )
    }
    pub fn activate(&self) {
        self.active.store(true, Ordering::SeqCst);
    }
    pub fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
    }
    pub fn submit(&self, activity: Activity) {
        if self.active.load(Ordering::SeqCst)
            && self
                .taken
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            && self.active.load(Ordering::SeqCst)
        {
            let _ = self.sender.try_send(activity);
        }
    }
}
