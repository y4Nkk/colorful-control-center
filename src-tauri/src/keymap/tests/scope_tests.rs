use super::*;
use crate::hal::{Hal, oem_dchu::Dchu};
use std::sync::{Arc, Mutex};
struct Shared {
    configuration: Mutex<Configuration>,
    flags: Mutex<[u8; 2]>,
    fail_flags: Mutex<bool>,
    effects: Mutex<Vec<String>>,
}
impl Shared {
    fn new() -> Self {
        Self {
            configuration: Mutex::new(original_config()),
            flags: Mutex::new([0, 1]),
            fail_flags: Mutex::new(false),
            effects: Mutex::new(vec![]),
        }
    }
}
impl Io for Shared {
    fn facts(&self) -> Result<Facts> {
        let mut capability = [0; 15];
        capability[0] = 1;
        capability[13] = 2;
        Ok(Facts {
            capability,
            flags: *self.flags.lock().unwrap(),
            original: Some(original::Source {
                name: "隔离原配置".into(),
                configuration: original_config(),
                fingerprint: "isolated-original".into(),
            }),
            original_error: None,
        })
    }
    fn ensure_available(&self) -> Result<()> {
        Ok(())
    }
    fn replace(&self, target: &Configuration) -> Result<()> {
        self.effects.lock().unwrap().push("mapping".into());
        let mut current = self.configuration.lock().unwrap();
        *current = model::merge(&current, target)?;
        Ok(())
    }
    fn save_flags(&self, flags: [u8; 2], scope: &[Device]) -> Result<()> {
        self.effects.lock().unwrap().push("flags".into());
        let mut current = self.flags.lock().unwrap();
        for device in scope {
            current[model::index(*device)] = flags[model::index(*device)];
        }
        if std::mem::take(&mut *self.fail_flags.lock().unwrap()) {
            bail!("隔离启用字段失败");
        }
        Ok(())
    }
    fn prepare_programs(&self, _: &Record, _: Arc<Mutex<()>>) -> Result<Option<launch::Session>> {
        self.effects.lock().unwrap().push("programs".into());
        Ok(None)
    }
}
unsafe extern "system" fn buffer(_: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn read_settings(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn set(_: i32, _: *const u8, _: i32) -> i32 {
    0
}
unsafe extern "system" fn write_settings(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    0
}
fn hal() -> Hal {
    Hal::test_instance(
        Dchu::test_io(buffer, read_settings, set, write_settings),
        None,
    )
}

#[test]
fn controller_without_injected_io_refuses_all_native_read_and_write_paths() {
    let dir = tempfile::tempdir().unwrap();
    let controller = Controller::new(dir.path().join("record.json"));
    let hal = hal();
    assert!(
        controller
            .read(&hal)
            .err()
            .unwrap()
            .to_string()
            .contains("未读取原厂配置")
    );
    assert!(
        controller
            .apply(
                &hal,
                &Request {
                    expected_revision: "old".into(),
                    configuration: target(Device::Keyboard)
                }
            )
            .is_err()
    );
    assert!(controller.restore(&hal, "old").is_err());
    assert!(controller.retry_programs(&hal, "old").is_err());
    assert!(controller.hotkey_identity(&hal).is_err());
    assert!(controller.toggle_owned(&hal, "old").is_err());
    assert!(!controller.path.exists());
}

#[test]
fn scoped_controller_and_hotkey_share_persistent_first_baseline_and_compensation() {
    let dir = tempfile::tempdir().unwrap();
    let memory = Arc::new(Shared::new());
    let controller = Controller::new(dir.path().join("record.json")).with_test_io(memory.clone());
    let hal = hal();
    let original = controller.read(&hal).unwrap().unwrap();
    assert!(!controller.path.exists());
    controller
        .apply(
            &hal,
            &Request {
                expected_revision: original.revision,
                configuration: target(Device::Keyboard),
            },
        )
        .unwrap();
    let identity = controller.hotkey_identity(&hal).unwrap();
    assert!(!controller.toggle_owned(&hal, &identity).unwrap());
    *memory.fail_flags.lock().unwrap() = true;
    assert!(controller.toggle_owned(&hal, &identity).is_err());
    assert_eq!(*memory.flags.lock().unwrap(), [0, 1]);
    let record = store::FileStore(controller.path.clone())
        .load()
        .unwrap()
        .record
        .unwrap();
    assert_eq!(record.baseline.configuration, original_config());
    assert_eq!(record.phase, Phase::Applied);
    let current = controller.read(&hal).unwrap().unwrap();
    controller.restore(&hal, &current.revision).unwrap();
    assert_eq!(*memory.configuration.lock().unwrap(), original_config());
    assert_eq!(*memory.flags.lock().unwrap(), [0, 1]);
}

#[test]
fn controller_reads_and_applies_serialize_with_domain_then_hal_lock() {
    let dir = tempfile::tempdir().unwrap();
    let memory = Arc::new(Shared::new());
    let controller = Controller::new(dir.path().join("record.json")).with_test_io(memory);
    let hal = hal();
    let request = Request {
        expected_revision: controller.read(&hal).unwrap().unwrap().revision,
        configuration: target(Device::Keyboard),
    };
    std::thread::scope(|scope| {
        let writing = scope.spawn(|| controller.apply(&hal, &request));
        let reading = scope.spawn(|| controller.read(&hal));
        writing.join().unwrap().unwrap();
        reading.join().unwrap().unwrap();
    });
    assert!(controller.read(&hal).unwrap().unwrap().can_restore);
}

#[test]
fn missing_program_factory_refuses_without_journal_or_filter_writes() {
    let dir = tempfile::tempdir().unwrap();
    let memory = Arc::new(Shared::new());
    let controller = Controller::new(dir.path().join("record.json")).with_test_io(memory.clone());
    let hal = hal();
    let mut configuration = target(Device::Keyboard);
    configuration.devices[0].bindings[0].action = Action::Launch {
        path: "isolated.exe".into(),
        event_id: 256,
    };
    let request = Request {
        expected_revision: controller.read(&hal).unwrap().unwrap().revision,
        configuration,
    };
    assert!(
        controller
            .apply(&hal, &request)
            .err()
            .unwrap()
            .to_string()
            .contains("没有准备所需监听")
    );
    assert!(!controller.path.exists());
    assert!(
        !memory
            .effects
            .lock()
            .unwrap()
            .iter()
            .any(|effect| effect == "mapping" || effect == "flags")
    );
}
