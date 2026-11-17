use crate::gpu_power::mode::State;
use anyhow::Result;
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) fn wait(
    mode: u8,
    node: Option<&State>,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    if mode != 4 || node.is_none_or(|state| state.target.enabled) {
        return Ok(());
    }
    // 原厂quiet_plus在关断独显时，模式命令之后等待1000ms再处理EnergyStar。
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        permit()?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(());
        }
        thread::sleep(remaining.min(Duration::from_millis(20)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_power::Target;
    fn state(enabled: bool) -> State {
        State {
            id: "ACPI\\NVDA0820\\ISOLATED".into(),
            name: "Memory".into(),
            target: Target::requested(enabled),
        }
    }
    #[test]
    fn other_modes_and_online_or_absent_nodes_have_no_settle_period() {
        let mut unexpected = || panic!("不适用的分支不能额外等待");
        for mode in 0..4 {
            wait(mode, Some(&state(false)), &mut unexpected).unwrap();
        }
        wait(4, Some(&state(true)), &mut unexpected).unwrap();
        wait(4, None, &mut unexpected).unwrap();
    }
    #[test]
    fn permission_is_checked_before_and_during_the_original_settle_period() {
        let mut calls = 0;
        let error = wait(4, Some(&state(false)), &mut || {
            calls += 1;
            if calls == 2 {
                anyhow::bail!("隔离等待中撤销");
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(calls, 2);
        assert!(error.to_string().contains("隔离等待中撤销"));
    }
    #[test]
    fn quiet_plus_waits_the_full_original_interval_with_permission_checks() {
        let started = Instant::now();
        let mut calls = 0;
        wait(4, Some(&state(false)), &mut || {
            calls += 1;
            Ok(())
        })
        .unwrap();
        assert!(started.elapsed() >= Duration::from_secs(1));
        assert!(calls >= 2);
    }
}
