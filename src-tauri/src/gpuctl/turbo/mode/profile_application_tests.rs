use super::*;
use crate::profiles::{Devices, ProfileFile, ProfileSettings};
#[path = "profile_authorization_tests.rs"]
mod authorization_tests;
#[path = "profile_persistence_tests.rs"]
mod persistence_tests;

struct Harness {
    f: Fixture,
    memory: Arc<Memory>,
    gpu: crate::gpuctl::GpuController,
    environment: Arc<EnvironmentFixture>,
    power: gpu_power::Controller,
    source: CombinedSource,
}
struct Preview {
    file: ProfileFile,
    mode: mode::Target,
    power: gpu_power::profile::Review,
}
impl Harness {
    fn new(off: bool) -> Self {
        let f = core().with_energy(2);
        let (memory, gpu, environment, power) = setup();
        if off {
            close_node(&environment);
            gpu.set_power_suspended(true).unwrap();
        }
        environment.io.0.lock().unwrap().device.automatic_power_off = true;
        let source = CombinedSource {
            memory: memory.clone(),
            device: f.device.clone(),
        };
        Self {
            f,
            memory,
            gpu,
            environment,
            power,
            source,
        }
    }
    fn devices(&self) -> Devices<'_> {
        Devices {
            mode_source: Some(&self.source),
            gpu_power: &self.power,
            ..self.f.devices()
        }
    }
    fn preview(&self) -> Preview {
        let node = self.environment.io.device();
        let settings = ProfileSettings {
            oem_power_mode: Some(1),
            gpu_power: Some(gpu_power::profile::Target {
                id: node.id,
                name: node.name,
                values: gpu_power::Target {
                    enabled: true,
                    automatic_power_off: false,
                },
            }),
            gpu: Some(crate::gpuctl::GpuClockSettings {
                core_offset_mhz: Some(60),
                mem_offset_mhz: None,
            }),
            pl1_w: Some(77.0),
            ..Default::default()
        };
        let session = context(&self.f, &self.memory, &self.gpu, &self.power)
            .session()
            .unwrap();
        let _gate = self.f.hal.begin_write().unwrap();
        let mode = crate::profiles::preview_mode(&session.context, &settings)
            .unwrap()
            .unwrap();
        let power = session
            .node_scope()
            .preview_profile(&self.f.hal, settings.gpu_power.as_ref().unwrap())
            .unwrap();
        Preview {
            file: ProfileFile {
                version: 1,
                name: "完整模式档案".into(),
                saved_at_ms: 1,
                settings,
            },
            mode,
            power,
        }
    }
    fn apply(&self, preview: &Preview) -> Result<Vec<crate::profiles::ApplyStep>> {
        crate::profiles::apply_reviewed(
            &self.f.hal,
            &self.gpu,
            self.devices(),
            &preview.file,
            &preview.file,
            Some(&preview.mode),
            Some(&preview.power),
            None,
        )
    }
}
fn success(steps: &[crate::profiles::ApplyStep]) {
    assert!(
        steps.iter().all(|step| step.ok),
        "{:?}",
        steps
            .iter()
            .map(|s| (&s.step, &s.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn reviewed_profile_entry_runs_three_stages_mode_clocks_and_independent_offsets_with_one_grant() {
    let h = Harness::new(true);
    let raw = RawState::read(h.memory.as_ref()).unwrap();
    let preview = h.preview();
    assert_eq!(
        preview
            .mode
            .gpu
            .as_ref()
            .unwrap()
            .target()
            .unwrap()
            .stages
            .as_ref()
            .unwrap()
            .requests()
            .len(),
        3
    );
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    success(&h.apply(&preview).unwrap());
    assert_eq!(h.environment.authorizations.load(Ordering::SeqCst), 1);
    assert!(h.environment.io.device().enabled && !h.environment.io.device().automatic_power_off);
    assert_eq!(h.f.device.0.lock().unwrap().pages[1][1], 1);
    assert_eq!(
        RawState::read(h.memory.as_ref()).unwrap().offsets[0].core_khz,
        Some(60_000)
    );
    let initial = full_profile(&h.f, &h.memory, &h.gpu, &h.power)
        .restore
        .unwrap()
        .gpu
        .unwrap();
    assert!(!initial.node.as_ref().unwrap().target.enabled);
    assert!(initial.node.as_ref().unwrap().target.automatic_power_off);
    assert_eq!(
        initial.clocks.as_ref().unwrap().origin,
        ClockOrigin::AfterEnable
    );
    assert_eq!(initial.clocks.unwrap().state, raw);
}

#[test]
fn late_profile_failure_restores_nodes_independent_offsets_mode_clocks_core_and_saved_power() {
    let h = Harness::new(true);
    h.f.display.enable();
    let raw = RawState::read(h.memory.as_ref()).unwrap();
    let saved = h.f.device.0.lock().unwrap().pages[6];
    let mut preview = h.preview();
    preview.file.settings.display = h.f.display.controller.capture().unwrap();
    let display = h.f.display.io.clone();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    h.environment.io.0.lock().unwrap().node_hook = Some(Arc::new(move |_| {
        if count.fetch_add(1, Ordering::SeqCst) == 2 {
            display.0.lock().unwrap().displays[0].current.frequency = 144;
        }
    }));
    let steps = h.apply(&preview).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "显示刷新率复核" && !step.ok)
    );
    assert!(
        steps
            .iter()
            .filter(|step| step.step.starts_with("恢复"))
            .all(|step| step.ok),
        "{:?}",
        steps
            .iter()
            .map(|step| (&step.step, &step.message))
            .collect::<Vec<_>>()
    );
    assert_eq!(h.environment.authorizations.load(Ordering::SeqCst), 1);
    assert_eq!(h.environment.io.device(), preview.power.before);
    assert_eq!(RawState::read(h.memory.as_ref()).unwrap(), raw);
    let current = h.f.device.0.lock().unwrap();
    assert_eq!(current.pages[1][1], 2);
    assert_eq!(current.pages[6], saved);
}

#[test]
fn deferred_driver_failure_after_profile_preparation_restores_off_before_unrelated_targets() {
    let h = Harness::new(true);
    let preview = h.preview();
    h.memory.0.lock().unwrap().fail_lock_read_at = Some(1);
    let steps = h.apply(&preview).unwrap();
    assert!(steps.iter().any(|step| !step.ok));
    assert_eq!(h.environment.io.device(), preview.power.before);
    assert_eq!(h.environment.authorizations.load(Ordering::SeqCst), 1);
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert!(h.f.hal.recovery().unwrap().oem_mode.is_none());
}

#[test]
fn changed_complete_device_after_profile_authorization_has_no_core_clock_or_recovery_output() {
    let h = Harness::new(false);
    let preview = h.preview();
    let io = h.environment.io.clone();
    h.environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || {
        let mut data = io.0.lock().unwrap();
        data.device.drives_display = true;
        data.device.can_disable = false;
    }));
    assert!(h.apply(&preview).is_err());
    assert_eq!(h.environment.authorizations.load(Ordering::SeqCst), 1);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
    assert!(h.f.hal.recovery().unwrap().oem_mode.is_none());
}

#[test]
fn missing_composite_mode_or_node_graph_confirmation_never_authorizes_the_profile() {
    let h = Harness::new(true);
    let mut preview = h.preview();
    preview.mode.gpu = None;
    assert!(h.apply(&preview).is_err());
    let mut preview = h.preview();
    if let Some(links::Review::Ready(gpu)) = preview.mode.gpu.as_mut() {
        gpu.stages = None;
    }
    assert!(h.apply(&preview).is_err());
    assert_eq!(h.environment.authorizations.load(Ordering::SeqCst), 0);
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
}
