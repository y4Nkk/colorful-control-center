use super::{Device, Target, mode, model, operation, request, sequence};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Stages {
    pub before: Device,
    #[serde(deserialize_with = "Option::deserialize")]
    pub mode: Option<u8>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub preparation: Option<request::Request>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub before_mode: Option<request::Request>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub after_mode: Option<request::Request>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub finish: Option<request::Request>,
}
impl Stages {
    pub(crate) fn build(
        before: &Device,
        mode: Option<u8>,
        explicit: Option<&Target>,
        needs_gpu: bool,
    ) -> Result<Self> {
        super::mode::validate_device(before)?;
        if let Some(target) = explicit {
            target.validate()?;
            if target.id != before.id {
                bail!("档案独显目标属于其他设备");
            }
            if needs_gpu && !target.values.enabled {
                bail!("档案GPU目标需要最终保持独显开启");
            }
        }
        let action = mode
            .map(|mode| super::mode::action(true, Some(before), mode, None, false))
            .transpose()?
            .flatten();
        let mut current = before.clone();
        let early = if mode.is_none() {
            explicit
                .filter(|target| target.values.enabled)
                .map(|target| target.values)
        } else if (needs_gpu || explicit.is_some_and(|target| target.values.enabled))
            && !before.enabled
        {
            Some(model::Target {
                enabled: true,
                automatic_power_off: before.automatic_power_off,
            })
        } else {
            None
        };
        let preparation = step(&mut current, early)?;
        let before_mode = step(
            &mut current,
            action
                .filter(|action| action.stage == mode::Stage::BeforeMode)
                .map(|action| action.target),
        )?;
        let after_mode = step(
            &mut current,
            action
                .filter(|action| action.stage == mode::Stage::AfterMode)
                .map(|action| action.target),
        )?;
        let final_target = explicit.map(|target| target.values).or_else(|| {
            needs_gpu.then_some(model::Target {
                enabled: true,
                automatic_power_off: before.automatic_power_off,
            })
        });
        let finish = step(&mut current, final_target)?;
        let stages = Self {
            before: before.clone(),
            mode,
            preparation,
            before_mode,
            after_mode,
            finish,
        };
        let requests = stages.requests();
        if !requests.is_empty() {
            sequence::Sequence::new(&requests)?;
        }
        Ok(stages)
    }
    pub(crate) fn requests(&self) -> Vec<request::Request> {
        [
            &self.preparation,
            &self.before_mode,
            &self.after_mode,
            &self.finish,
        ]
        .into_iter()
        .filter_map(|request| request.clone())
        .collect()
    }
    pub(crate) fn mode_before(&self) -> Device {
        self.preparation
            .as_ref()
            .map(|request| request.expected.projected(request.target))
            .unwrap_or_else(|| self.before.clone())
    }
    pub(crate) fn final_device(&self) -> Device {
        self.requests()
            .last()
            .map(|request| request.expected.projected(request.target))
            .unwrap_or_else(|| self.before.clone())
    }
}
fn step(current: &mut Device, target: Option<model::Target>) -> Result<Option<request::Request>> {
    let Some(target) = target.filter(|target| *target != current.target()) else {
        return Ok(None);
    };
    operation::preflight(current, current.target(), target)?;
    let request = request::Request {
        expected: current.clone(),
        target,
        restore: false,
    };
    request.encoded()?;
    *current = current.projected(target);
    Ok(Some(request))
}

#[cfg(test)]
#[path = "stages_tests.rs"]
mod tests;
