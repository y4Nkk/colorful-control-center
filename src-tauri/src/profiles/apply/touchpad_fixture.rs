use super::Device;
use crate::touchpad::{self, Snapshot};
use anyhow::{Result, bail};
use std::sync::{Arc, Mutex};

pub struct Memory {
    device: Arc<Device>,
    pub devices: Mutex<Vec<String>>,
    pub enabled: Mutex<Option<bool>>,
    pub writes: Mutex<Vec<String>>,
    pub fail_at: Mutex<Vec<usize>>,
}
impl Memory {
    pub fn new(device: Arc<Device>) -> Arc<Self> {
        Arc::new(Self {
            device,
            devices: Mutex::new(vec![]),
            enabled: Mutex::new(None),
            writes: Mutex::new(vec![]),
            fail_at: Mutex::new(vec![]),
        })
    }
    pub fn enable(&self) {
        *self.devices.lock().unwrap() = vec!["ACPI\\I2C0001\\0".into()];
        *self.enabled.lock().unwrap() = Some(true);
        self.device.0.lock().unwrap().pages[1][10] = 0;
    }
    fn result(&self) -> Result<()> {
        if self
            .fail_at
            .lock()
            .unwrap()
            .contains(&self.writes.lock().unwrap().len())
        {
            bail!("隔离触摸板部分失败");
        }
        Ok(())
    }
}
impl touchpad::Io for Memory {
    fn snapshot(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            devices: self.devices.lock().unwrap().clone(),
            enabled: *self.enabled.lock().unwrap(),
            saved: self.device.0.lock().unwrap().pages[1][10],
        })
    }
    fn ensure_available(&self) -> Result<()> {
        Ok(())
    }
    fn set_enabled(&self, devices: &[String], before: bool, enabled: bool) -> Result<()> {
        if *self.devices.lock().unwrap() != devices || *self.enabled.lock().unwrap() != Some(before)
        {
            bail!("隔离触摸板来源变化");
        }
        *self.enabled.lock().unwrap() = Some(enabled);
        self.writes
            .lock()
            .unwrap()
            .push(format!("enabled:{enabled}"));
        self.device
            .0
            .lock()
            .unwrap()
            .events
            .push(format!("TP:{enabled}"));
        self.result()
    }
    fn save(&self, value: u8) -> Result<()> {
        self.device.0.lock().unwrap().pages[1][10] = value;
        self.writes.lock().unwrap().push(format!("saved:{value}"));
        self.result()
    }
}
