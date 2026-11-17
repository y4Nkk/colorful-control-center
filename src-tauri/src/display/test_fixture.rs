//! 显式内存显示环境；没有原生提供者或生产进程回退。
use super::*;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Memory(pub Arc<Mutex<Data>>);
#[derive(Default)]
pub struct Data {
    pub displays: Vec<Display>,
    pub writes: Vec<Mode>,
    pub fail_at: Vec<usize>,
}
pub struct Fixture {
    pub io: Memory,
    pub controller: Controller,
}
impl Fixture {
    pub fn new() -> Self {
        let io = Memory::default();
        Self {
            controller: Controller::with_guards(io.clone(), guardian::MemoryFactory),
            io,
        }
    }
    pub fn enable(&self) {
        self.io.0.lock().unwrap().displays = (1..=2)
            .map(|index| Display {
                current: Mode {
                    device: format!("DISPLAY{index}"),
                    identity: format!("panel-{index}"),
                    name: format!("Panel {index}"),
                    width: 1920,
                    height: 1080,
                    bits: 32,
                    orientation: 0,
                    x: (index - 1) * 1920,
                    y: 0,
                    flags: 0,
                    frequency: 60,
                },
                frequencies: vec![60, 144],
                primary: index == 1,
            })
            .collect();
    }
}
impl Access for Memory {
    fn list(&self) -> Result<Vec<Display>> {
        Ok(self.0.lock().unwrap().displays.clone())
    }
    fn test(&self, _: &Mode) -> Result<()> {
        Ok(())
    }
    fn set(&self, mode: &Mode) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        let display = data
            .displays
            .iter_mut()
            .find(|display| display.current.device == mode.device)
            .ok_or_else(|| anyhow!("模拟显示器已断开"))?;
        if display.current.identity != mode.identity {
            bail!("模拟显示器已替换");
        }
        display.current = mode.clone();
        data.writes.push(mode.clone());
        if data.fail_at.contains(&data.writes.len()) {
            bail!("模拟显示写入失败");
        }
        Ok(())
    }
}
