use super::super::store::{FileStore, Store};
use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
struct Queue {
    events: Mutex<Vec<event_log::Record>>,
    cutovers: AtomicUsize,
}
struct Input(Arc<Queue>);
impl runtime::Input for Input {
    fn poll(&mut self) -> Result<Vec<event_log::Record>> {
        Ok(std::mem::take(&mut *self.0.events.lock().unwrap()))
    }
    fn discard_pending(&mut self) -> Result<()> {
        self.0.events.lock().unwrap().clear();
        self.0.cutovers.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[derive(Default)]
pub(crate) struct TestPrograms {
    entries: Mutex<Vec<(Arc<runtime::Control>, Arc<Queue>)>>,
    pub fail_prepare: AtomicBool,
    pub prepared: AtomicUsize,
    pub exited: Arc<AtomicUsize>,
    pub exited_without_gate: Arc<AtomicUsize>,
    pub launched: Arc<Mutex<Vec<String>>>,
    pub recording: Arc<AtomicBool>,
    pub prepare_hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
impl TestPrograms {
    pub fn prepare(
        &self,
        path: PathBuf,
        record: Record,
        gate: Arc<Mutex<()>>,
    ) -> Result<Option<Session>> {
        if self.fail_prepare.load(Ordering::SeqCst) {
            bail!("隔离程序监听准备失败");
        }
        let programs = programs(&record);
        if programs.is_empty() {
            return Ok(None);
        }
        let control = Arc::new(runtime::Control::default());
        let queue = Arc::new(Queue::default());
        let (shared, input, expected) = (control.clone(), queue.clone(), record.clone());
        let (launched, recording, exited, free_gate) = (
            self.launched.clone(),
            self.recording.clone(),
            self.exited.clone(),
            self.exited_without_gate.clone(),
        );
        let worker = thread::spawn(move || {
            let result = runtime::run(&mut Input(input), &shared, &gate, |event| {
                dispatch(
                    event,
                    &programs,
                    &expected,
                    || Ok(recording.load(Ordering::SeqCst)),
                    || Ok(FileStore(path.clone()).load()?.record),
                    |program| {
                        launched.lock().unwrap().push(program.into());
                        Ok(())
                    },
                )?;
                Ok(())
            });
            if let Err(error) = result {
                shared.fail(error);
            } else {
                shared.pause();
            }
            if gate.try_lock().is_ok() {
                free_gate.fetch_add(1, Ordering::SeqCst);
            }
            exited.fetch_add(1, Ordering::SeqCst);
        });
        let session = Session {
            control: control.clone(),
            thread: Some(worker),
            record,
        };
        self.entries.lock().unwrap().push((control, queue));
        self.prepared.fetch_add(1, Ordering::SeqCst);
        let hook = self.prepare_hook.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(Some(session))
    }
    pub fn event(&self, event_id: u16) {
        for (_, queue) in self.entries.lock().unwrap().iter() {
            queue.events.lock().unwrap().push(event_log::Record {
                number: 1,
                event_id: 0x40000000 | u32::from(event_id),
                event_type: 4,
                source: "HKKbdFltr".into(),
                strings: vec![],
            });
        }
    }
    pub fn active(&self) -> usize {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|(control, _)| control.accepts(control.epoch.load(Ordering::SeqCst)))
            .count()
    }
    pub fn cutovers(&self) -> usize {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|(_, queue)| queue.cutovers.load(Ordering::SeqCst))
            .sum()
    }
    pub fn fail(&self, index: usize) {
        self.entries.lock().unwrap()[index]
            .0
            .fail(anyhow!("隔离程序事件源失效"));
    }
}
