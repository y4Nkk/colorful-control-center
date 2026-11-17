use super::Device;
use crate::microphone::{self, Endpoint, Snapshot};
use anyhow::{Result, bail};
use std::sync::{Arc, Mutex};

pub struct Memory {
    pub device: Arc<Device>,
    pub endpoints: Mutex<Vec<Endpoint>>,
    pub writes: Mutex<Vec<String>>,
    pub fail_at: Mutex<Vec<usize>>,
    pub blocked: Mutex<bool>,
}
impl Memory {
    pub fn new(device: Arc<Device>) -> Arc<Self> {
        Arc::new(Self {
            device,
            endpoints: Mutex::new(vec![]),
            writes: Mutex::new(vec![]),
            fail_at: Mutex::new(vec![]),
            blocked: Mutex::new(false),
        })
    }
    fn result(&self) -> Result<()> {
        let count = self.writes.lock().unwrap().len();
        if self.fail_at.lock().unwrap().contains(&count) {
            bail!("隔离麦克风部分失败");
        }
        Ok(())
    }
    pub fn enable(&self) {
        *self.endpoints.lock().unwrap() = vec![
            Endpoint {
                id: "A".into(),
                name: Some("内置麦克风".into()),
                muted: false,
            },
            Endpoint {
                id: "B".into(),
                name: Some("USB麦克风".into()),
                muted: true,
            },
        ];
    }
}
impl microphone::Io for Memory {
    fn snapshot(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            endpoints: self.endpoints.lock().unwrap().clone(),
            saved: self.device.0.lock().unwrap().pages[1][41],
        })
    }
    fn ensure_available(&self) -> Result<()> {
        if *self.blocked.lock().unwrap() {
            bail!("隔离原厂麦克风控制者仍在运行");
        }
        Ok(())
    }
    fn set_muted(&self, id: &str, muted: bool) -> Result<()> {
        let mut endpoints = self.endpoints.lock().unwrap();
        let endpoint = endpoints
            .iter_mut()
            .find(|endpoint| endpoint.id == id)
            .ok_or_else(|| anyhow::anyhow!("隔离麦克风已断开"))?;
        endpoint.muted = muted;
        self.writes.lock().unwrap().push(format!("{id}:{muted}"));
        self.device
            .0
            .lock()
            .unwrap()
            .events
            .push(format!("MIC:{id}:{muted}"));
        self.result()
    }
    fn save(&self, value: u8) -> Result<()> {
        self.device.0.lock().unwrap().pages[1][41] = value;
        self.writes.lock().unwrap().push(format!("saved:{value}"));
        self.result()
    }
}
