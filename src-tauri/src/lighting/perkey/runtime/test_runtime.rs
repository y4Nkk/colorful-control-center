use super::super::Io;
use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
pub(crate) struct TestRuntime {
    records: Mutex<Vec<(Arc<Control>, mpsc::SyncSender<Envelope>)>>,
    pub fail_prepare: AtomicBool,
    pub prepared: AtomicUsize,
    pub exited: Arc<AtomicUsize>,
    pub exit_with_gate: Arc<AtomicUsize>,
    pub prepare_hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
impl TestRuntime {
    pub(crate) fn prepare(
        &self,
        values: &Values,
        device: &DeviceInfo,
        identity: String,
        gate: Arc<Mutex<()>>,
        io: Arc<dyn Io + Send + Sync>,
    ) -> Result<Option<Session>> {
        if self.fail_prepare.load(Ordering::SeqCst) {
            bail!("隔离逐键事件准备失败");
        }
        let control = Arc::new(Control::default());
        let (tx, rx) = mpsc::sync_channel(64);
        let (shared, config, info, exited) = (
            control.clone(),
            values.clone(),
            device.clone(),
            self.exited.clone(),
        );
        let exit_with_gate = self.exit_with_gate.clone();
        let worker = thread::spawn(move || {
            run(rx, &shared, &gate, &config, &info, |packet| io.send(packet));
            if gate.try_lock().is_ok() {
                exit_with_gate.fetch_add(1, Ordering::SeqCst);
            }
            exited.fetch_add(1, Ordering::SeqCst);
        });
        self.records.lock().unwrap().push((control.clone(), tx));
        self.prepared.fetch_add(1, Ordering::SeqCst);
        let session = Session {
            control,
            threads: vec![worker],
            values: values.clone(),
            identity,
            device: device.clone(),
            input: None,
        };
        let hook = self.prepare_hook.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(Some(session))
    }
    pub(crate) fn event(&self, scan: u16, down: bool) {
        for (control, tx) in self.records.lock().unwrap().iter() {
            control.enqueue(tx, KeyEvent { scan, down });
        }
    }
    pub(crate) fn active(&self) -> usize {
        self.records
            .lock()
            .unwrap()
            .iter()
            .filter(|(control, _)| control.accepts(control.epoch.load(Ordering::SeqCst)))
            .count()
    }
    pub(crate) fn fail(&self, index: usize) {
        self.records.lock().unwrap()[index]
            .0
            .fail("隔离事件源失效".into());
    }
}
