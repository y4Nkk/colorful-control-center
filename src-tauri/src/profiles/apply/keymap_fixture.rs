use super::*;
use crate::keymap;
use ccc_core::keymap::{Action, Binding, Configuration, Device, DeviceSettings};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

pub struct Backend {
    pub device: Arc<super::super::fixtures::Device>,
    pub source: Mutex<Option<keymap::original::Source>>,
    pub current: Mutex<Configuration>,
    pub programs: Arc<keymap::TestPrograms>,
    pub path: PathBuf,
    pub blocked: AtomicBool,
    pub fail_replace: AtomicUsize,
    pub fail_flags: AtomicUsize,
    pub lock_commit: AtomicBool,
    pub commit_lock: Mutex<Option<std::fs::File>>,
}
impl keymap::Io for Backend {
    fn facts(&self) -> Result<keymap::Facts> {
        let state = self.device.0.lock().unwrap();
        Ok(keymap::Facts {
            capability: state.bios[..15].try_into().unwrap(),
            flags: [state.pages[1][8], state.pages[1][9]],
            original: self.source.lock().unwrap().clone(),
            original_error: None,
        })
    }
    fn ensure_available(&self) -> Result<()> {
        if self.blocked.load(Ordering::SeqCst) {
            bail!("隔离原厂控制者存在");
        }
        Ok(())
    }
    fn replace(&self, target: &Configuration) -> Result<()> {
        {
            let mut current = self.current.lock().unwrap();
            for device in &target.devices {
                *current
                    .devices
                    .iter_mut()
                    .find(|before| before.device == device.device)
                    .unwrap() = device.clone();
            }
        }
        self.device
            .0
            .lock()
            .unwrap()
            .events
            .push("KEYMAP:replace".into());
        if self
            .fail_replace
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            bail!("映射部分写入失败");
        }
        Ok(())
    }
    fn save_flags(&self, flags: [u8; 2], scope: &[Device]) -> Result<()> {
        let mut state = self.device.0.lock().unwrap();
        for device in scope {
            let at = if *device == Device::Keyboard { 0 } else { 1 };
            state.pages[1][8 + at] = flags[at];
        }
        state.events.push("KEYMAP:flags".into());
        if self.lock_commit.load(Ordering::SeqCst) {
            use std::os::windows::fs::OpenOptionsExt;
            let mut held = self.commit_lock.lock().unwrap();
            if held.is_none() {
                *held = Some(
                    std::fs::OpenOptions::new()
                        .read(true)
                        .share_mode(0)
                        .open(&self.path)?,
                );
            }
        }
        if self
            .fail_flags
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            bail!("映射启用保存失败");
        }
        Ok(())
    }
    fn prepare_programs(
        &self,
        record: &keymap::Record,
        gate: Arc<Mutex<()>>,
    ) -> Result<Option<keymap::Session>> {
        self.programs
            .prepare(self.path.clone(), record.clone(), gate)
    }
}
pub struct Harness {
    pub f: Fixture,
    pub io: Arc<Backend>,
    pub programs: Arc<keymap::TestPrograms>,
    pub path: PathBuf,
    _dir: tempfile::TempDir,
}
pub fn original() -> Configuration {
    Configuration {
        devices: [Device::Keyboard, Device::Mouse]
            .into_iter()
            .map(|device| DeviceSettings {
                device,
                enabled: device == Device::Mouse,
                bindings: vec![Binding {
                    trigger: if device == Device::Keyboard { 30 } else { 4 },
                    action: Action::Disable,
                }],
            })
            .collect(),
    }
}
impl Harness {
    pub fn new() -> Self {
        let mut f = Fixture::new();
        {
            let mut state = f.device.0.lock().unwrap();
            state.bios[0] = 1;
            state.bios[13] = 2;
            state.pages[1][9] = 1;
            state.pages[2][4] = 1;
            state.pages[2][35] = 3;
            state.pages[2][36..40].copy_from_slice(&[1, 0, 0, 30]);
            state.pages[2][80] = 1;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keymap-recovery.json");
        let programs = Arc::new(keymap::TestPrograms::default());
        let io = Arc::new(Backend {
            device: f.device.clone(),
            source: Mutex::new(Some(keymap::original::Source {
                name: "隔离原厂完整配置".into(),
                configuration: original(),
                fingerprint: "original-full".into(),
            })),
            current: Mutex::new(original()),
            programs: programs.clone(),
            path: path.clone(),
            blocked: AtomicBool::new(false),
            fail_replace: AtomicUsize::new(0),
            fail_flags: AtomicUsize::new(0),
            lock_commit: AtomicBool::new(false),
            commit_lock: Mutex::new(None),
        });
        f.keymap = keymap::Controller::new(path.clone()).with_test_io(io.clone());
        Self {
            f,
            io,
            programs,
            path,
            _dir: dir,
        }
    }
    pub fn wanted(&self, programs_enabled: bool) -> ProfileSettings {
        ProfileSettings {
            keymap: Some(keymap::Target {
                configuration: Configuration {
                    devices: vec![DeviceSettings {
                        device: Device::Keyboard,
                        enabled: true,
                        bindings: vec![Binding {
                            trigger: 31,
                            action: Action::Launch {
                                path: r"F:\isolated\helper.exe".into(),
                                event_id: 256,
                            },
                        }],
                    }],
                },
                programs_enabled,
            }),
            ..settings()
        }
    }
    pub fn apply(&self, wanted: &ProfileSettings) -> Result<Vec<ApplyStep>> {
        apply_settings(&self.f.hal, &self.f.gpu, self.f.devices(), wanted)
    }
    pub fn record(&self) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap()
    }
    pub fn profile(&self) -> serde_json::Value {
        serde_json::to_value(self.f.keymap.read(&self.f.hal).unwrap().unwrap()).unwrap()
    }
    pub fn running(&self) -> bool {
        self.profile()["programs"]["running"] == true
    }
    pub fn revision(&self) -> String {
        self.profile()["revision"].as_str().unwrap().into()
    }
    pub fn start_old(&self) {
        assert!(
            self.apply(&self.wanted(true))
                .unwrap()
                .iter()
                .all(|step| step.ok)
        );
        wait_until(|| self.running());
        self.f.device.0.lock().unwrap().events.clear();
    }
}
pub fn wait_until(check: impl Fn() -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !check() {
        assert!(
            std::time::Instant::now() < until,
            "程序会话未按时达到目标状态"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
pub fn gpu_failure(h: &Harness, wanted: &mut ProfileSettings) {
    h.f.device.0.lock().unwrap().fail_gpu = true;
    wanted.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
}
