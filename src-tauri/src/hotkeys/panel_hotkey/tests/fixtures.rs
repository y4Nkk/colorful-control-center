use super::super::*;
use crate::hal::panel_inventory::{Inventory, Monitor};
use std::{collections::VecDeque, sync::Arc};

pub(super) fn inventory(oled: bool) -> Inventory {
    let mut edid = [0u8; 128];
    edid[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
    edid[8..12].copy_from_slice(if oled {
        &[0x4c, 0x83, 0x29, 0xa0]
    } else {
        &[1, 2, 3, 4]
    });
    edid[127] = 0u8.wrapping_sub(edid[..127].iter().fold(0u8, |s, v| s.wrapping_add(*v)));
    Inventory {
        monitors: vec![
            Monitor::from_edid(
                "DISPLAY\\fixture".into(),
                "隔离面板".into(),
                vec!["fixture".into()],
                &edid,
                0,
                0,
            )
            .unwrap(),
        ],
        unresolved: vec![],
    }
}
pub(super) fn source(oled: bool) -> Source {
    let saver = oled.then(|| model::Saver {
        path: r"C:\isolated\blank-fixture.exe".into(),
        fingerprint: "a".repeat(64),
    });
    Source::new(inventory(oled), Some(&[0, 0]), saver).unwrap()
}
pub(super) struct Data {
    pub source: Source,
    pub broken: bool,
    pub after_prepare: Option<Source>,
    pub prepare_fail: bool,
    pub start_fail: bool,
    pub stop_fail: bool,
    pub poll_fail: bool,
    pub wake_fail: bool,
    pub reads: usize,
    pub prepares: usize,
    pub starts: usize,
    pub stops: usize,
    pub wakes: usize,
    pub events: VecDeque<Event>,
}
#[derive(Clone)]
pub(super) struct Memory(pub Arc<Mutex<Data>>);
impl Memory {
    pub fn new(oled: bool) -> Self {
        Self(Arc::new(Mutex::new(Data {
            source: source(oled),
            broken: false,
            after_prepare: None,
            prepare_fail: false,
            start_fail: false,
            stop_fail: false,
            poll_fail: false,
            wake_fail: false,
            reads: 0,
            prepares: 0,
            starts: 0,
            stops: 0,
            wakes: 0,
            events: VecDeque::new(),
        })))
    }
    pub fn id(&self) -> String {
        self.0.lock().unwrap().source.identity().unwrap()
    }
    pub fn begin(&self, controller: &Controller) -> Result<Option<String>> {
        controller.begin(self, &self.id(), || Ok(true), || Ok(true))
    }
}
impl Io for Memory {
    fn current(&self) -> Result<Source> {
        let mut data = self.0.lock().unwrap();
        data.reads += 1;
        if data.broken {
            bail!("面板读取失败");
        }
        Ok(data.source.clone())
    }
    fn prepare(&self, _: &Source) -> Result<Box<dyn Resource>> {
        let mut data = self.0.lock().unwrap();
        data.prepares += 1;
        if data.prepare_fail {
            bail!("准备资源失败");
        }
        if let Some(source) = data.after_prepare.take() {
            data.source = source;
        }
        Ok(Box::new(self.clone()))
    }
    fn wake(&self, _: &Source) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.wakes += 1;
        if data.wake_fail {
            bail!("唤醒请求失败");
        }
        Ok(())
    }
}
impl Resource for Memory {
    fn start(&mut self) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.starts += 1;
        if data.start_fail {
            bail!("启动资源失败");
        }
        Ok(())
    }
    fn poll(&mut self) -> Result<Event> {
        let mut data = self.0.lock().unwrap();
        if data.poll_fail {
            bail!("资源状态读取失败");
        }
        Ok(data.events.pop_front().unwrap_or(Event::Pending))
    }
    fn stop(&mut self) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.stops += 1;
        if data.stop_fail {
            bail!("结束资源失败");
        }
        Ok(())
    }
}
