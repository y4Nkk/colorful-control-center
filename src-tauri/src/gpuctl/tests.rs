use super::*;
use crate::hal::nvapi::pstates::{DomainReading, ParsedPstates};
use anyhow::anyhow;

#[test]
fn reconnect_after_power_cycle_uses_only_explicitly_injected_source() {
    let io = MockSession::new(101_500, 202_500);
    let controller = GpuController::test_with_session(io.clone());
    controller.set_power_suspended(true).unwrap();
    controller.set_power_suspended(false).unwrap();
    assert_eq!(controller.checkpoint().unwrap().core_khz, Some(101_500));
    assert!(controller.apply_offsets(Some(60), None).ok);
    assert_eq!(io.writes().last().unwrap().0, Some(60_000));
    let missing = GpuController::new();
    assert!(
        missing
            .checkpoint()
            .unwrap_err()
            .to_string()
            .contains("禁止连接真实驱动")
    );
}

#[test]
fn power_suspend_drops_session_and_blocks_reconnect_but_keeps_raw_baseline() {
    let io = MockSession::new(101_500, 202_500);
    let controller = GpuController::test_with_session(io);
    controller.state.lock().unwrap().baseline = Some(GpuCheckpoint {
        device: *test_identity::Identity::default().0.lock().unwrap(),
        core_khz: Some(101_500),
        mem_khz: Some(202_500),
    });
    controller.set_power_suspended(true).unwrap();
    let profile = controller.read_profile();
    assert!(!profile.available);
    assert!(profile.unavailable_reason.unwrap().contains("调校已暂停"));
    let state = controller.state.lock().unwrap();
    assert!(state.session.is_none());
    assert_eq!(state.baseline.unwrap().core_khz, Some(101_500));
    assert_eq!(state.baseline.unwrap().mem_khz, Some(202_500));
}

struct MockSession {
    inner: Mutex<MockInner>,
}
struct MockInner {
    core_khz: i32,
    mem_khz: i32,
    writes: Vec<(Option<i32>, Option<i32>)>,
}
impl MockSession {
    fn new(core_khz: i32, mem_khz: i32) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(MockInner {
                core_khz,
                mem_khz,
                writes: Vec::new(),
            }),
        })
    }
    fn parsed(&self) -> ParsedPstates {
        let domain = |delta| DomainReading {
            present: true,
            editable: true,
            delta_khz: delta,
            min_khz: -500_000,
            max_khz: 900_000,
        };
        let inner = self.inner.lock().unwrap();
        ParsedPstates {
            editable: true,
            p0_present: true,
            core: domain(inner.core_khz),
            memory: domain(inner.mem_khz),
            count_clamped: false,
        }
    }
    fn writes(&self) -> Vec<(Option<i32>, Option<i32>)> {
        self.inner.lock().unwrap().writes.clone()
    }
}
impl PstatesIo for MockSession {
    fn read(&self) -> Result<ParsedPstates> {
        Ok(self.parsed())
    }
    fn write(&self, core_khz: Option<i32>, mem_khz: Option<i32>) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.writes.push((core_khz, mem_khz));
        if let Some(v) = core_khz {
            inner.core_khz = v;
        }
        if let Some(v) = mem_khz {
            inner.mem_khz = v;
        }
        Ok(())
    }
}
fn controller_with(session: Arc<MockSession>) -> GpuController {
    let controller = GpuController::test_with_session(session);
    controller.state.lock().unwrap().gpu_name = Some("Mock RTX".into());
    controller
}
#[test]
fn read_profile_maps_facts() {
    let controller = controller_with(MockSession::new(135_000, 500_000));
    let p = controller.read_profile();
    assert!(p.available);
    assert_eq!(p.gpu_name.as_deref(), Some("Mock RTX"));
    assert_eq!(p.core_offset_mhz, Some(135));
    assert_eq!(p.mem_offset_mhz, Some(500));
    assert!(p.core_editable && p.mem_editable);
    assert_eq!((p.core_min_mhz, p.core_max_mhz), (-300, 300));
    assert_eq!((p.mem_min_mhz, p.mem_max_mhz), (-500, 900));
    assert!(p.baseline.is_none());
}
#[test]
fn apply_records_baseline_only_on_first_success() {
    let session = MockSession::new(135_000, 0);
    let c = controller_with(session.clone());
    let first = c.apply_offsets(Some(150), Some(500));
    assert!(first.ok, "{}", first.message);
    assert_eq!(first.core_offset_mhz, Some(150));
    assert_eq!(first.mem_offset_mhz, Some(500));
    let initial = Some(GpuClockSettings {
        core_offset_mhz: Some(135),
        mem_offset_mhz: Some(0),
    });
    assert_eq!(c.read_profile().baseline, initial);
    assert!(c.apply_offsets(Some(200), Some(0)).ok);
    assert_eq!(c.read_profile().baseline, initial);
    assert_eq!(session.writes().len(), 2);
}
#[test]
fn restore_writes_baseline_back() {
    let session = MockSession::new(50_000, 200_000);
    let c = controller_with(session.clone());
    assert!(c.apply_offsets(Some(100), Some(400)).ok);
    let outcome = c.restore_baseline();
    assert!(outcome.ok, "{}", outcome.message);
    assert_eq!(outcome.core_offset_mhz, Some(50));
    assert_eq!(outcome.mem_offset_mhz, Some(200));
    assert!(c.read_profile().baseline.is_some());
    assert_eq!(
        session.writes(),
        vec![
            (Some(100_000), Some(400_000)),
            (Some(50_000), Some(200_000))
        ]
    );
}
#[test]
fn restore_without_baseline_fails_cleanly() {
    let c = controller_with(MockSession::new(0, 0));
    let outcome = c.restore_baseline();
    assert!(!outcome.ok);
    assert!(outcome.message.contains("没有可恢复的基准"));
}

#[test]
fn replaced_gpu_cannot_receive_another_devices_baseline_or_checkpoint() {
    let session = MockSession::new(50_000, 200_000);
    let identity = Arc::new(test_identity::Identity::default());
    let controller = GpuController::test_with_sources(session.clone(), identity.clone());
    let before = controller.checkpoint().unwrap();
    assert!(controller.apply_offsets(Some(100), Some(400)).ok);
    identity.0.lock().unwrap().bus += 1;
    let writes = session.writes().len();
    let profile = controller.read_profile();
    assert!(profile.available && !profile.core_editable && profile.baseline.is_none());
    assert!(!controller.apply_offsets(Some(110), None).ok);
    assert!(!controller.restore_baseline().ok);
    assert!(controller.restore_checkpoint(&before).is_err());
    assert_eq!(session.writes().len(), writes);
}

#[test]
fn failed_offset_rollback_keeps_device_bound_baseline_for_retry() {
    struct Partial(Mutex<(i32, usize)>);
    impl PstatesIo for Partial {
        fn read(&self) -> Result<ParsedPstates> {
            let value = self.0.lock().unwrap().0;
            Ok(ParsedPstates {
                editable: true,
                p0_present: true,
                count_clamped: false,
                core: DomainReading {
                    present: true,
                    editable: true,
                    delta_khz: value,
                    min_khz: -300_000,
                    max_khz: 300_000,
                },
                memory: DomainReading::default(),
            })
        }
        fn write(&self, core: Option<i32>, _: Option<i32>) -> Result<()> {
            let mut state = self.0.lock().unwrap();
            if let Some(value) = core {
                state.0 = value;
            }
            state.1 += 1;
            if state.1 <= 2 {
                return Err(anyhow!("模拟部分写入失败"));
            }
            Ok(())
        }
    }
    let io = Arc::new(Partial(Mutex::new((25_500, 0))));
    let controller = GpuController::test_with_session(io.clone());
    let outcome = controller.apply_offsets(Some(100), None);
    assert!(!outcome.ok && outcome.rollback_error.is_some());
    assert_eq!(
        controller.state.lock().unwrap().baseline.unwrap().core_khz,
        Some(25_500)
    );
    assert!(controller.restore_baseline().ok);
    assert_eq!(io.0.lock().unwrap().0, 25_500);
}
#[test]
fn capture_settings_reads_current() {
    let c = controller_with(MockSession::new(135_000, 500_000));
    assert_eq!(
        c.capture_settings().unwrap(),
        GpuClockSettings {
            core_offset_mhz: Some(135),
            mem_offset_mhz: Some(500)
        }
    );
}
#[test]
fn checkpoint_restores_noninteger_mhz_exactly() {
    let session = MockSession::new(135_500, 200_200);
    let c = controller_with(session.clone());
    let before = c.checkpoint().unwrap();
    assert!(c.apply_offsets(Some(150), Some(300)).ok);
    c.restore_checkpoint(&before).unwrap();
    assert_eq!(
        session.writes().last(),
        Some(&(Some(135_500), Some(200_200)))
    );
}
#[test]
fn apply_settings_delegates_same_path() {
    let c = controller_with(MockSession::new(0, 0));
    let outcome = c.apply_settings(&GpuClockSettings {
        core_offset_mhz: Some(30),
        mem_offset_mhz: None,
    });
    assert!(outcome.ok, "{}", outcome.message);
    assert_eq!(outcome.core_offset_mhz, Some(30));
    assert_eq!(outcome.mem_offset_mhz, Some(0));
}
#[test]
fn capture_without_p0_reports_error() {
    struct NoP0;
    impl PstatesIo for NoP0 {
        fn read(&self) -> Result<ParsedPstates> {
            Ok(ParsedPstates::default())
        }
        fn write(&self, _: Option<i32>, _: Option<i32>) -> Result<()> {
            Err(anyhow!("不应写"))
        }
    }
    let c = GpuController::test_with_session(Arc::new(NoP0));
    assert!(
        c.capture_settings()
            .unwrap_err()
            .to_string()
            .contains("缺少 P0")
    );
    let p = c.read_profile();
    assert!(p.available);
    assert!(!p.core_editable && !p.mem_editable);
    assert!(p.warnings.iter().any(|w| w.contains("缺少 P0")));
}
#[test]
fn serde_settings_are_camel_case() {
    let json = serde_json::to_string(&GpuClockSettings {
        core_offset_mhz: Some(1),
        mem_offset_mhz: None,
    })
    .unwrap();
    assert_eq!(json, r#"{"coreOffsetMhz":1,"memOffsetMhz":null}"#);
}
