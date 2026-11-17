use super::*;
use crate::lighting::perkey;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
pub struct Traffic {
    pub packets: Vec<[u8; 5]>,
    pub writes: usize,
    pub fail_sends: Vec<usize>,
    pub fail_writes: Vec<usize>,
}
pub struct Memory {
    pub device: Arc<super::super::fixtures::Device>,
    pub info: Mutex<perkey::DeviceInfo>,
    pub identity: Mutex<String>,
    pub traffic: Mutex<Traffic>,
    pub watched_runtime: Mutex<Option<Arc<perkey::TestRuntime>>>,
    pub active_during_write: AtomicUsize,
}
impl perkey::Io for Memory {
    fn identity(&self) -> String {
        self.identity.lock().unwrap().clone()
    }
    fn device(&self) -> perkey::DeviceInfo {
        self.info.lock().unwrap().clone()
    }
    fn read(&self, page: i32, offset: i32, length: usize) -> Result<Vec<u8>> {
        Ok(self.device.0.lock().unwrap().pages[page as usize]
            [offset as usize..offset as usize + length]
            .to_vec())
    }
    fn write(&self, page: i32, offset: i32, bytes: &[u8]) -> Result<()> {
        let fail = {
            let mut state = self.traffic.lock().unwrap();
            state.writes += 1;
            state.fail_writes.contains(&state.writes)
        };
        let mut state = self.device.0.lock().unwrap();
        state.events.push(format!("PERKEY_SAVE:{page}:{offset}"));
        state.pages[page as usize][offset as usize..offset as usize + bytes.len()]
            .copy_from_slice(bytes);
        if fail {
            bail!("逐键保存部分写入失败");
        }
        Ok(())
    }
    fn send(&self, packet: [u8; 5]) -> Result<()> {
        let fail = {
            let mut state = self.traffic.lock().unwrap();
            state.packets.push(packet);
            state.fail_sends.contains(&state.packets.len())
        };
        if let Some(runtime) = self.watched_runtime.lock().unwrap().as_ref() {
            self.active_during_write
                .fetch_max(runtime.active(), Ordering::SeqCst);
        }
        if fail {
            bail!("逐键即时传输失败");
        }
        Ok(())
    }
}
pub struct Harness {
    pub f: Fixture,
    pub io: Arc<Memory>,
    pub runtime: Arc<perkey::TestRuntime>,
}
impl Harness {
    pub fn new() -> Self {
        let mut f = Fixture::new();
        {
            let mut state = f.device.0.lock().unwrap();
            state.pages[2][4] = 3;
            state.bios[0] = 1;
            state.pages[2][32..36].copy_from_slice(&[12, 1, 2, 4]);
            state.pages[2][36..41].copy_from_slice(&[1, 0, 0, 30, 0]);
            state.pages[2][80] = 1;
            state.pages[2][128..].fill(17);
            state.pages[3].fill(17);
        }
        let runtime = Arc::new(perkey::TestRuntime::default());
        let io = Arc::new(Memory {
            device: f.device.clone(),
            info: Mutex::new(perkey::DeviceInfo {
                new_effects: true,
                language: 0,
                narrow_border: true,
                three_column: false,
                compact: Some(false),
                physical_three_column: Some(false),
            }),
            identity: Mutex::new("isolated-hid-A".into()),
            traffic: Mutex::new(Traffic::default()),
            watched_runtime: Mutex::new(Some(runtime.clone())),
            active_during_write: AtomicUsize::new(0),
        });
        f.perkey = perkey::Controller::default()
            .with_test_io(io.clone())
            .with_test_runtime(runtime.clone());
        Self { f, io, runtime }
    }
    pub fn target(&self, reactive: bool) -> perkey::Target {
        let color = serde_json::from_value(serde_json::json!({"r":10,"g":20,"b":30})).unwrap();
        perkey::Target {
            keyboard_type: 3,
            device: self.io.info.lock().unwrap().clone(),
            values: perkey::Values {
                enabled: true,
                brightness: 3,
                speed: 1,
                pattern: if reactive {
                    perkey::Pattern::Ripple {
                        reactive: true,
                        color: Some(color),
                    }
                } else {
                    perkey::Pattern::Static {
                        colors: vec![color; 120],
                    }
                },
            },
        }
    }
    pub fn settings(&self, reactive: bool) -> ProfileSettings {
        ProfileSettings {
            perkey_lighting: Some(self.target(reactive)),
            ..settings()
        }
    }
    pub fn apply(&self, settings: &ProfileSettings) -> Result<Vec<ApplyStep>> {
        apply_settings(&self.f.hal, &self.f.gpu, self.f.devices(), settings)
    }
    pub fn start_old(&self) {
        let current = self.f.perkey.read(&self.f.hal).unwrap();
        let mut target = self.target(true);
        target.values.brightness = 2;
        self.f
            .perkey
            .apply(
                &self.f.hal,
                &perkey::Request {
                    expected_revision: current.revision,
                    values: target.values,
                },
            )
            .unwrap();
        assert_eq!(self.runtime.active(), 1);
        self.io.active_during_write.store(0, Ordering::SeqCst);
        self.f.device.0.lock().unwrap().events.clear();
    }
    pub fn pages(&self) -> [[u8; 256]; 2] {
        let state = self.f.device.0.lock().unwrap();
        [state.pages[2], state.pages[3]]
    }
}
pub fn failing_gpu(settings: &mut ProfileSettings) {
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
}
pub fn wait_until(check: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !check() {
        assert!(
            std::time::Instant::now() < deadline,
            "隔离事件线程未按时完成"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
