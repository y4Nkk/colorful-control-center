use super::{
    Values,
    device::DeviceInfo,
    events::{Engine, KeyEvent},
    hid,
};
use crate::hal::key_events;
use anyhow::{Result, anyhow, bail};
use std::{
    sync::{
        Arc, Mutex, TryLockError,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[cfg(test)]
#[path = "runtime/test_runtime.rs"]
mod test_runtime;
#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use test_runtime::TestRuntime;

#[derive(Default)]
pub(super) struct Control {
    active: AtomicBool,
    stop: AtomicBool,
    overflow: AtomicBool,
    epoch: AtomicU64,
    input_stop: Mutex<Option<key_events::Stopper>>,
    error: Mutex<Option<String>>,
}
impl Control {
    fn pause(&self) {
        self.active.store(false, Ordering::SeqCst);
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }
    fn fail(&self, error: String) {
        if let Ok(mut value) = self.error.lock() {
            *value = Some(error);
        }
        self.stop.store(true, Ordering::SeqCst);
        self.pause();
        if let Ok(stop) = self.input_stop.lock() {
            if let Some(stop) = stop.as_ref() {
                stop.stop();
            }
        }
    }
    fn accepts(&self, epoch: u64) -> bool {
        !self.stop.load(Ordering::SeqCst)
            && !self.overflow.load(Ordering::SeqCst)
            && self.active.load(Ordering::SeqCst)
            && self.epoch.load(Ordering::SeqCst) == epoch
    }
}
pub(super) struct Envelope {
    epoch: u64,
    event: KeyEvent,
}
pub struct Session {
    control: Arc<Control>,
    threads: Vec<JoinHandle<()>>,
    pub values: Values,
    identity: String,
    device: DeviceInfo,
    input: Option<key_events::Session>,
}
impl Session {
    pub fn prepare(
        values: &Values,
        device: &DeviceInfo,
        identity: String,
        gate: Arc<Mutex<()>>,
    ) -> Result<Option<Self>> {
        if !values.enabled || !values.pattern.reactive() {
            return Ok(None);
        }
        let control = Arc::new(Control::default());
        let (tx, rx) = mpsc::sync_channel(64);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (config, info, shared) = (values.clone(), device.clone(), control.clone());
        let target_identity = identity.clone();
        let worker = thread::Builder::new()
            .name("perkey-output".into())
            .spawn(move || {
                let keyboard = hid::Keyboard::open().and_then(|keyboard| {
                    if keyboard.identity() != target_identity {
                        bail!("按键灯效设备身份变化");
                    }
                    Ok(keyboard)
                });
                let _ = ready_tx.send(keyboard.as_ref().map(|_| ()).map_err(|e| format!("{e:#}")));
                if let Ok(keyboard) = keyboard {
                    run(rx, &shared, &gate, &config, &info, |packet| {
                        keyboard.send(packet)
                    });
                }
            })?;
        let mut session = Self {
            control,
            threads: vec![worker],
            values: values.clone(),
            identity,
            device: device.clone(),
            input: None,
        };
        ready_rx
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| anyhow!("按键灯效输出准备超时"))?
            .map_err(anyhow::Error::msg)?;
        let input = keyboard_input(tx, session.control.clone())?;
        *session
            .control
            .input_stop
            .lock()
            .map_err(|_| anyhow!("按键事件停止状态异常"))? = Some(input.stopper());
        session.input = Some(input);
        Ok(Some(session))
    }
    pub fn ready(&self) -> Result<()> {
        if self.control.stop.load(Ordering::SeqCst)
            || self.control.overflow.load(Ordering::SeqCst)
            || self.threads.iter().any(|thread| thread.is_finished())
        {
            bail!(
                "逐键事件会话已停止：{}",
                self.error().unwrap_or_else(|| "输出线程不可用".into())
            );
        }
        Ok(())
    }
    pub fn activate(&self) -> Result<()> {
        self.ready()?;
        self.control.active.store(true, Ordering::SeqCst);
        Ok(())
    }
    pub fn pause(&self) {
        self.control.pause();
    }
    pub fn running(&self) -> bool {
        self.control
            .accepts(self.control.epoch.load(Ordering::SeqCst))
    }
    pub fn error(&self) -> Option<String> {
        self.control.error.lock().ok().and_then(|e| e.clone())
    }
    pub fn matches(&self, values: &Values, identity: &str, device: &DeviceInfo) -> bool {
        self.values == *values && self.identity == identity && self.device == *device
    }
}
fn keyboard_input(
    sender: mpsc::SyncSender<Envelope>,
    control: Arc<Control>,
) -> Result<key_events::Session> {
    let failed = control.clone();
    key_events::Session::start(
        move |key| {
            let epoch = control.epoch.load(Ordering::SeqCst);
            if control.accepts(epoch)
                && sender
                    .try_send(Envelope {
                        epoch,
                        event: KeyEvent {
                            scan: key.scan,
                            down: key.down,
                        },
                    })
                    .is_err()
            {
                control.overflow.store(true, Ordering::SeqCst);
            }
            false
        },
        move |error| failed.fail(error),
    )
}
impl Drop for Session {
    fn drop(&mut self) {
        self.control.pause();
        self.control.stop.store(true, Ordering::SeqCst);
        if let Ok(stop) = self.control.input_stop.lock() {
            if let Some(stop) = stop.as_ref() {
                stop.stop();
            }
        }
        self.input.take();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}
fn run(
    rx: Receiver<Envelope>,
    control: &Control,
    gate: &Mutex<()>,
    values: &Values,
    device: &DeviceInfo,
    mut send: impl FnMut([u8; 5]) -> Result<()>,
) {
    let start = Instant::now();
    let mut engine = Engine::default();
    let mut epoch = control.epoch.load(Ordering::SeqCst);
    while !control.stop.load(Ordering::SeqCst) {
        if control.overflow.load(Ordering::SeqCst) {
            control.fail("按键事件队列已满，灯效已停止；请重新应用".into());
            break;
        }
        let next = match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let current = control.epoch.load(Ordering::SeqCst);
        if current != epoch {
            engine = Engine::default();
            epoch = current;
        }
        if !control.accepts(epoch) || next.as_ref().is_some_and(|e| e.epoch != epoch) {
            continue;
        }
        let packets = match next {
            Some(e) => engine.event(e.event, start.elapsed(), &values.pattern, device),
            None => engine.tick(start.elapsed(), &values.pattern),
        };
        if let Err(e) = output(control, gate, epoch, packets, &mut send) {
            control.fail(format!("按键灯效发送失败：{e:#}"));
        }
    }
}
fn output(
    control: &Control,
    gate: &Mutex<()>,
    epoch: u64,
    packets: Vec<[u8; 5]>,
    send: &mut impl FnMut([u8; 5]) -> Result<()>,
) -> Result<()> {
    if packets.is_empty() {
        return Ok(());
    }
    loop {
        if !control.accepts(epoch) {
            return Ok(());
        }
        match gate.try_lock() {
            Ok(_guard) => {
                if control.accepts(epoch) {
                    for packet in packets {
                        if !control.accepts(epoch) {
                            break;
                        }
                        send(packet)?;
                    }
                }
                return Ok(());
            }
            Err(TryLockError::WouldBlock) => thread::sleep(Duration::from_millis(5)),
            Err(TryLockError::Poisoned(_)) => bail!("控制事务异常，按键灯效已停止"),
        }
    }
}
