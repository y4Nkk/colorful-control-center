use super::Fixture;
use crate::{gpu_power::profile, hal::NvidiaSnapshot};

impl Fixture {
    pub fn enable_gpu_power(&mut self) {
        self.gpu_environment.io.0.lock().unwrap().supported = true;
        let io = self.gpu_environment.io.clone();
        self.hal.test_nvidia = Some(std::sync::Arc::new(move || {
            let data = io.0.lock().unwrap();
            Ok(data.device.enabled.then(|| NvidiaSnapshot {
                name: "Injected GPU".into(),
                temp_c: data.temperature,
                ..Default::default()
            }))
        }));
    }
    pub fn gpu_target(&self, enabled: bool) -> profile::Target {
        let device = self.gpu_environment.io.device();
        profile::Target {
            id: device.id,
            name: device.name,
            values: crate::gpu_power::Target::requested(enabled),
        }
    }
}
