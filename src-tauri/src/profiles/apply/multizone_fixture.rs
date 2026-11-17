use super::fixtures::{Device, Fixture};
use crate::lighting::multizone::{self, Capabilities, Snapshot, Target, Values};
use anyhow::{Result, bail};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct Traffic {
    pub packets: Vec<[u8; 37]>,
    pub saves: usize,
    pub fail_sends: Vec<usize>,
    pub fail_saves: Vec<usize>,
    pub blocked: bool,
}
pub(super) struct Memory {
    pub device: Arc<Device>,
    pub traffic: Mutex<Traffic>,
}
impl multizone::Io for Memory {
    fn snapshot(&self) -> Result<Snapshot> {
        let data = self.device.0.lock().unwrap();
        let snapshot = Snapshot {
            capabilities: Capabilities {
                keyboard_type: data.pages[2][4],
                light_bar_switchable: data.bios[23] & 128 != 0,
                light_bar_enabled: true,
                cover: data.bios[23] & 32 != 0,
            },
            page: data.pages[2].to_vec(),
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
    fn ensure_available(&self) -> Result<()> {
        if self.traffic.lock().unwrap().blocked {
            bail!("隔离多区控制者冲突");
        }
        Ok(())
    }
    fn send(&self, packet: &[u8; 37]) -> Result<()> {
        let mut traffic = self.traffic.lock().unwrap();
        traffic.packets.push(*packet);
        let failed = traffic.fail_sends.contains(&traffic.packets.len());
        drop(traffic);
        self.device
            .0
            .lock()
            .unwrap()
            .events
            .push(format!("MULTI:{:?}", &packet[..3]));
        if failed {
            bail!("隔离多区发送部分失败");
        }
        Ok(())
    }
    fn save(&self, patches: &[ccc_core::settings::Patch]) -> Result<()> {
        let mut traffic = self.traffic.lock().unwrap();
        traffic.saves += 1;
        let failed = traffic.fail_saves.contains(&traffic.saves);
        drop(traffic);
        let mut data = self.device.0.lock().unwrap();
        for patch in patches {
            data.pages[2][patch.offset..patch.offset + patch.data.len()]
                .copy_from_slice(&patch.data);
            if failed {
                bail!("隔离多区保存部分失败");
            }
        }
        Ok(())
    }
}
pub(super) fn enable(f: &mut Fixture) -> Arc<Memory> {
    {
        let mut data = f.device.0.lock().unwrap();
        data.pages[2][4] = 23;
        data.bios[..2].copy_from_slice(&[1, 0]);
        data.bios[23] = 32;
        data.pages[2][32] = 1;
        data.pages[2][34] = 3;
        data.pages[2][35] = 4;
        data.pages[2][80] = 1;
        for at in [85, 89, 93, 10, 27, 13] {
            data.pages[2][at..at + 3].copy_from_slice(&[1, 2, 3]);
        }
    }
    let io = Arc::new(Memory {
        device: f.device.clone(),
        traffic: Mutex::new(Traffic::default()),
    });
    f.multi_zone = multizone::Controller::default().with_test_io(io.clone());
    io
}
pub(super) fn target(f: &Fixture) -> Target {
    let profile = f.multi_zone.read(&f.hal).unwrap().unwrap();
    let mut values: Values = profile.current.unwrap();
    values.mode = 4;
    values.speed = 1;
    values.brightness = 2;
    values.zones[3].r = 99;
    Target {
        capabilities: profile.capabilities,
        values,
    }
}
