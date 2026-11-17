use super::*;
use std::sync::{Arc, Mutex};

pub(crate) struct Data {
    pub supported: bool,
    pub observed: Observed,
    pub identity: Identity,
    pub sent: Vec<u8>,
    pub fail_send: Vec<usize>,
    pub reboots: usize,
    pub hook: Option<Arc<dyn Fn(u8) + Send + Sync>>,
}
pub(crate) struct Memory(pub Mutex<Data>);
impl Io for Memory {
    fn observed(&self) -> Result<Option<Observed>> {
        let data = self.0.lock().unwrap();
        Ok(data.supported.then(|| data.observed.clone()))
    }
    fn identity(&self) -> Result<Identity> {
        Ok(self.0.lock().unwrap().identity.clone())
    }
    fn send(&self, target: u8) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.sent.push(target);
        let failed = data.fail_send.contains(&data.sent.len());
        let hook = data.hook.clone();
        drop(data);
        if let Some(hook) = hook {
            hook(target);
        }
        if failed {
            bail!("隔离显卡选择可能已发送后失败");
        }
        Ok(())
    }
    fn reboot(&self) -> Result<()> {
        self.0.lock().unwrap().reboots += 1;
        Ok(())
    }
}
pub(crate) struct Fixture {
    pub controller: Controller,
    pub io: Arc<Memory>,
    pub dir: tempfile::TempDir,
}
impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let io = Arc::new(Memory(Mutex::new(Data {
            supported: false,
            observed: Observed {
                mode: 3,
                available_mask: 15,
            },
            identity: Identity {
                owner: "a".repeat(64),
                boot: "1".repeat(32),
            },
            sent: Vec::new(),
            fail_send: Vec::new(),
            reboots: 0,
            hook: None,
        })));
        let mut controller = Controller::new(dir.path().join("mode.json"));
        controller.io = Some(io.clone());
        Self {
            controller,
            io,
            dir,
        }
    }
}
