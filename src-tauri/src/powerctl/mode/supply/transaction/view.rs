use super::{FileStore, Io, Record, Review, Scope, Snapshot, capture};
use crate::powerctl::mode::{
    supply::{digest, plans},
    workflow::RecoveryPreview,
};
use crate::{
    gpuctl::turbo::mode::ClockObservation,
    powerctl::mode::supply::record::{ClockRead, Inspection},
};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct View {
    pub revision: String,
    pub review: Option<Review>,
    pub enabled: bool,
    pub can_enable: bool,
    pub can_restore: bool,
    pub recovery: Option<RecoveryPreview>,
    pub error: Option<String>,
    pub recovery_error: Option<String>,
    pub archive_revision: Option<String>,
    #[serde(skip)]
    pub(in crate::powerctl::mode::supply) current: Option<Snapshot>,
}

pub(in crate::powerctl::mode::supply) fn read(io: &impl Io, store: &FileStore) -> Result<View> {
    read_scoped(io, store, Scope::All)
}
pub(in crate::powerctl::mode::supply) fn read_scoped(
    io: &impl Io,
    store: &FileStore,
    scope: Scope,
) -> Result<View> {
    let (stored, archive_revision) = match store.inspect() {
        Ok(Inspection::Missing) => (Ok(None), None),
        Ok(Inspection::Current(record)) => (Ok(Some(record)), None),
        Ok(Inspection::Invalid { error, revision }) => (Err(error), Some(revision)),
        Err(error) => (Err(error), None),
    };
    let (review, record, device, mut error) = evidence(io, stored, scope);
    let device_matches = record
        .as_ref()
        .zip(device.as_ref())
        .is_some_and(|(record, device)| &record.device_identity == device);
    let current = match current(io, record.as_ref(), review.as_ref(), scope) {
        Ok(value) => value,
        Err(issue) => {
            error = Some(format!("读取供电模式恢复前态：{issue:#}"));
            None
        }
    };
    if record.as_ref().is_some_and(|record| {
        record.enabled
            && record.attempt.is_none()
            && record
                .last
                .as_ref()
                .zip(current.as_ref())
                .is_some_and(|(a, b)| !a.agrees_with(b))
    }) {
        error = Some(
            "模式、风扇、电源计划、EnergyStar、独显或GPU频率已被外部修改，执行前须重新核对对应状态"
                .into(),
        );
    }
    let recovery = recovery(record.as_ref(), current.as_ref())?;
    let clock_unavailable = current
        .as_ref()
        .and_then(|current| match &current.gpu_clocks {
            ClockObservation::Unavailable(error) => Some(format!("GPU频率恢复前态不可用：{error}")),
            _ => None,
        });
    if let Some(issue) = &clock_unavailable {
        error = Some(issue.clone());
    }
    let recovery_error =
        recovery_device_issue(io, record.as_ref(), current.as_ref()).or(clock_unavailable);
    let revision = digest(&(&review, &record, &device, &current, &error, &recovery_error))?;
    let restorable =
        record.as_ref().is_some_and(|record| match &record.initial {
            None => true,
            Some(initial) => current.as_ref().is_some_and(|now| {
                match (initial.mode.energy(), now.mode.energy()) {
                    (Ok(a), Ok(b)) => a.hardware_p_state == b.hardware_p_state,
                    _ => false,
                }
            }),
        });
    Ok(View {
        enabled: record.as_ref().is_some_and(|record| record.enabled),
        can_enable: record.is_none()
            && error.is_none()
            && review.as_ref().is_some_and(|review| {
                review.on_ac.is_some()
                    && review.unavailable_for(scope).is_none()
                    && Some(&review.device_identity) == device.as_ref()
            }),
        can_restore: device_matches && restorable && recovery_error.is_none(),
        recovery,
        review,
        revision,
        error,
        current,
        recovery_error,
        archive_revision,
    })
}

fn recovery_device_issue(
    io: &impl Io,
    record: Option<&Record>,
    current: Option<&Snapshot>,
) -> Option<String> {
    let initial = record.and_then(|record| record.initial.as_ref());
    let gpu_error = initial
        .zip(current)
        .and_then(|(initial, current)| {
            io.review_gpu_restore(initial.gpu_power.as_ref(), current.gpu_power.as_ref())
                .err()
        })
        .map(|error| format!("独显恢复条件：{error:#}"));
    let clock_error = initial
        .and_then(|initial| initial.gpu_clocks.as_ref())
        .zip(current.and_then(|current| current.gpu_clocks.raw()))
        .filter(|(initial, actual)| initial.state.devices != actual.devices)
        .map(|_| "GPU频率恢复设备已变化，不能向新显卡发送旧值".to_string());
    gpu_error.or(clock_error)
}

fn current(
    io: &impl Io,
    record: Option<&Record>,
    review: Option<&Review>,
    scope: Scope,
) -> Result<Option<Snapshot>> {
    let Some(record) = record else {
        return Ok(None);
    };
    let Some((target, initial)) = record.capture_target.as_ref().zip(record.initial.as_ref())
    else {
        return Ok(None);
    };
    let read = match scope {
        Scope::All => ClockRead::preview(record.track_clocks),
        Scope::Event(trigger) => match review.and_then(|review| {
            review
                .on_ac
                .and_then(|on_ac| review.target(trigger, on_ac).map(|target| (review, target)))
        }) {
            Some((review, target)) => super::clock_capture(review, target, trigger)?.1,
            None => ClockRead::preview(record.track_clocks),
        },
    };
    capture(io, target, &initial.plans, read).map(Some)
}

fn evidence(
    io: &impl Io,
    record: Result<Option<Record>>,
    scope: Scope,
) -> (
    Option<Review>,
    Option<Record>,
    Option<String>,
    Option<String>,
) {
    let review = io.review(scope);
    let device = io.device_identity();
    let mut error = match (&review, &record) {
        (_, Err(error)) | (Err(error), _) => Some(format!("{error:#}")),
        (Ok(Some(review)), Ok(Some(record)))
            if review.device_identity != record.device_identity =>
        {
            Some("设备或模式能力已变化，保留恢复记录".into())
        }
        (Ok(Some(review)), Ok(Some(record)))
            if record.enabled && review.identity != record.identity =>
        {
            Some("供电模式目标已变化，旧许可不可执行".into())
        }
        (_, Ok(Some(record))) if record.attempt.is_some() => {
            Some("上次供电模式事务未完成，请确认恢复".into())
        }
        _ => None,
    };
    if let Err(issue) = &device {
        error.get_or_insert_with(|| format!("设备身份读取失败：{issue:#}"));
    }
    let review = review.ok().flatten();
    let record = record.ok().flatten();
    let device = device.ok();
    if error.is_none() {
        if let Some((review, record)) = review.as_ref().zip(record.as_ref()) {
            if record.enabled {
                error = review
                    .check_clocks(scope, &record.clock_approval)
                    .err()
                    .map(|error| format!("{error:#}"));
            }
        }
    }
    if record
        .as_ref()
        .is_some_and(|record| Some(&record.device_identity) != device.as_ref())
    {
        error = Some("设备或模式能力已变化，保留恢复记录".into());
    }
    if record.is_some() && review.is_none() && error.is_none() {
        error = Some("原厂供电模式能力不可用".into());
    }
    (review, record, device, error)
}

fn recovery(
    record: Option<&Record>,
    current: Option<&Snapshot>,
) -> Result<Option<RecoveryPreview>> {
    let Some((initial, current)) = record
        .and_then(|record| record.initial.as_ref())
        .zip(current)
    else {
        return Ok(None);
    };
    let mut preview = initial.mode.recovery_preview(&current.mode)?;
    preview.plan_changes = plans::changes(&initial.plans, &current.plans)?;
    preview.current_gpu_power = current.gpu_power.clone();
    preview.initial_gpu_power = initial.gpu_power.clone();
    preview.current_gpu_clocks = Some(current.gpu_clocks.clone());
    preview.initial_gpu_clocks = initial.gpu_clocks.clone();
    Ok(Some(preview))
}
