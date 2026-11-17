//! 电源通知的类型和只读传输；事件消费者负责策略，不在窗口回调里执行设备操作。
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(not(test))]
mod native;
#[cfg(not(test))]
pub(crate) use native::{Session, Stopper};

pub(super) const SUPPLY: u128 = 0x5d3e9a59_e9d5_4b00_a6bd_ff34ff516548;
pub(super) const DISPLAY: u128 = 0x2b84c20e_ad23_4ddf_93db_05ffbd7efca5;

fn window_class_name() -> Result<Vec<u16>> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| anyhow::anyhow!("电源通知窗口标识耗尽"))?;
    Ok(format!("ColorfulControlCenter.PowerEvents.{id}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Source {
    Ac,
    Battery,
    Ups,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Source(Source),
    Suspend,
    Resume,
    DisplayAwake(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DisplayPolicy {
    Allowed,
    DeepS5Enabled,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Route {
    pub modern_standby: Option<bool>,
    pub display_events: bool,
    pub warnings: Vec<String>,
}

fn route(modern_standby: Option<bool>, policy: DisplayPolicy) -> Route {
    let mut warnings = Vec::new();
    if modern_standby.is_none() {
        warnings.push("未能确认系统待机能力，仅使用系统挂起和恢复通知".into());
    }
    if modern_standby == Some(true) {
        match policy {
            DisplayPolicy::Allowed => (),
            DisplayPolicy::DeepS5Enabled => {
                warnings.push("Deep S5已开启，按原厂规则跳过显示关闭／开启分支".into())
            }
            DisplayPolicy::Unknown => {
                warnings.push("Deep S5条件未知，仅使用系统挂起和恢复通知".into())
            }
        }
    }
    Route {
        modern_standby,
        display_events: modern_standby == Some(true) && policy == DisplayPolicy::Allowed,
        warnings,
    }
}

fn decode(id: u128, bytes: &[u8]) -> Result<Option<Event>> {
    if ![SUPPLY, DISPLAY].contains(&id) {
        return Ok(None);
    }
    let Ok(bytes) = <[u8; 4]>::try_from(bytes) else {
        bail!("电源通知长度无效");
    };
    let value = u32::from_le_bytes(bytes);
    Ok(Some(if id == SUPPLY {
        Event::Source(match value {
            0 => Source::Ac,
            1 => Source::Battery,
            2 => Source::Ups,
            _ => Source::Unknown,
        })
    } else {
        match value {
            0 => Event::DisplayAwake(false),
            1 | 2 => Event::DisplayAwake(true),
            _ => bail!("显示电源通知状态无效"),
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_sessions_get_distinct_owned_window_classes() {
        let first = window_class_name().unwrap();
        let second = window_class_name().unwrap();
        assert_ne!(first, second);
        assert_eq!(first.last(), Some(&0));
        assert!(
            String::from_utf16(&first[..first.len() - 1])
                .unwrap()
                .starts_with("ColorfulControlCenter.PowerEvents.")
        );
        let workers: Vec<_> = (0..24)
            .map(|_| std::thread::spawn(window_class_name))
            .collect();
        let classes: std::collections::BTreeSet<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap().unwrap())
            .collect();
        assert_eq!(classes.len(), 24);
    }
    #[test]
    fn native_payload_decode_keeps_ups_unknown_and_dim_distinct() {
        for (value, source) in [
            (0u32, Source::Ac),
            (1, Source::Battery),
            (2, Source::Ups),
            (255, Source::Unknown),
        ] {
            assert_eq!(
                decode(SUPPLY, &value.to_le_bytes()).unwrap(),
                Some(Event::Source(source))
            );
        }
        for (value, awake) in [(0u32, false), (1, true), (2, true)] {
            assert_eq!(
                decode(DISPLAY, &value.to_le_bytes()).unwrap(),
                Some(Event::DisplayAwake(awake))
            );
        }
        assert!(decode(DISPLAY, &3u32.to_le_bytes()).is_err());
        for len in 0..8 {
            if len != 4 {
                assert!(decode(SUPPLY, &vec![0; len]).is_err());
            }
        }
        assert!(decode(0, &[]).unwrap().is_none());
    }
    #[test]
    fn display_route_requires_modern_standby_and_confirmed_deep_s5_conditions() {
        for modern in [None, Some(false), Some(true)] {
            for policy in [
                DisplayPolicy::Allowed,
                DisplayPolicy::DeepS5Enabled,
                DisplayPolicy::Unknown,
            ] {
                let route = route(modern, policy);
                assert_eq!(
                    route.display_events,
                    modern == Some(true) && policy == DisplayPolicy::Allowed
                );
                if modern.is_none() || (modern == Some(true) && policy != DisplayPolicy::Allowed) {
                    assert!(!route.warnings.is_empty());
                }
            }
        }
    }
}
