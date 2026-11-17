use super::{Candidate, Policy, RawState, Review, execution};
use crate::{
    gpuctl::{
        GpuController,
        turbo::{
            ensure_turbo,
            model::{self, DeviceReading},
            source::SourceIo,
            transaction,
        },
    },
    hal::nvapi::turbo::TurboIo,
};
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
pub(super) const DEFERRED_REASON: &str =
    "独显已关闭；驱动范围和实际频率在已确认模式事务的写前检查点核对";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DriverReview {
    pub identity: String,
    pub current: RawState,
    pub devices: Vec<DeviceReading>,
}
impl Review {
    fn needs_driver(&self, modes: [Option<u8>; 6]) -> bool {
        self.candidates()
            .iter()
            .zip(modes)
            .any(|(candidate, mode)| {
                mode.is_some()
                    && candidate
                        .decision
                        .as_ref()
                        .is_some_and(|decision| decision.target.is_some())
            })
    }
    pub(in crate::gpuctl::turbo) fn inspect_driver(
        &mut self,
        io: &dyn TurboIo,
        source: &dyn SourceIo,
        modes: [Option<u8>; 6],
        on_ac: Option<bool>,
    ) -> Result<()> {
        let actual_source = on_ac.ok_or_else(|| anyhow!("实际供电未知，不能检查GPU驱动目标"))?;
        let (snapshot, driver) = capture(io, source, &self.policy, actual_source)?;
        let candidates = [
            &mut self.ac,
            &mut self.battery,
            &mut self.startup_ac,
            &mut self.startup_battery,
            &mut self.resume_ac,
            &mut self.resume_battery,
        ];
        for ((candidate, mode), on_ac) in candidates
            .into_iter()
            .zip(modes)
            .zip([true, false, true, false, true, false])
        {
            if mode.is_none() {
                continue;
            }
            inspect_candidate(&snapshot, &driver.current, candidate, mode.unwrap(), on_ac);
        }
        self.driver = Some(driver);
        Ok(())
    }
}
pub(super) fn capture(
    io: &dyn TurboIo,
    source: &dyn SourceIo,
    policy: &Policy,
    on_ac: bool,
) -> Result<(model::Snapshot, DriverReview)> {
    let snapshot = model::capture_with_base(io, source.read()?, false)?;
    execution::check_source(&snapshot.source, policy, on_ac)?;
    execution::bind_devices(policy, &snapshot.devices)?;
    let driver = DriverReview {
        identity: snapshot.identity()?,
        current: RawState::from_snapshot(&snapshot)?,
        devices: snapshot.device_readings(),
    };
    Ok((snapshot, driver))
}
pub(super) fn inspect_candidate(
    snapshot: &model::Snapshot,
    current: &RawState,
    candidate: &mut Candidate,
    mode: u8,
    on_ac: bool,
) {
    let Some(target) = candidate
        .decision
        .as_ref()
        .and_then(|decision| decision.target.as_ref())
    else {
        return;
    };
    let checked: Result<RawState> = (|| {
        let mut future = snapshot.clone();
        future.source.mode = mode;
        future.source.on_ac = Some(on_ac);
        let planned = transaction::planned_state(&future, 97)?;
        if planned != current.projected(target)? {
            bail!("驱动预检与原厂模式目标不一致");
        }
        Ok(planned)
    })();
    match checked {
        Ok(planned) => candidate.projected = Some(planned),
        Err(error) => candidate.unavailable_reason = Some(format!("{error:#}")),
    }
}
impl GpuController {
    pub(crate) fn inspect_mode_gpu_in_control(
        &self,
        review: &mut Review,
        source: &dyn SourceIo,
        modes: [Option<u8>; 6],
        on_ac: Option<bool>,
        powered_off: bool,
    ) {
        if !review.needs_driver(modes) {
            return;
        }
        if powered_off {
            review.deferred_reason = Some(DEFERRED_REASON.into());
            return;
        }
        let result = (|| {
            let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
            review.inspect_driver(ensure_turbo(&mut state)?.as_ref(), source, modes, on_ac)
        })();
        if let Err(error) = result {
            review.driver_error = Some(format!("{error:#}"));
        }
    }
}
