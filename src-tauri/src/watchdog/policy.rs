use anyhow::{Result, bail};
use ccc_core::safety::RecoveryReason;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardPolicy {
    Protected,
    FirmwareFullSpeed,
}

impl GuardPolicy {
    pub fn for_mode(mode: u8) -> Self {
        if mode == 1 {
            Self::FirmwareFullSpeed
        } else {
            Self::Protected
        }
    }
}

pub struct Policy {
    full_speed: bool,
    pending: Option<Instant>,
}

impl Policy {
    pub fn new(policy: GuardPolicy) -> Self {
        Self {
            full_speed: false,
            pending: (policy == GuardPolicy::FirmwareFullSpeed).then(Instant::now),
        }
    }

    pub fn prepare(&mut self, full_speed: bool, reason: Option<&RecoveryReason>) -> Result<()> {
        if !full_speed && reason.is_some() {
            bail!("温度达到95°C保护阈值或传感器异常，保留当前全速散热");
        }
        if full_speed {
            // 同一事务重复准备不能延长尚未确认的豁免时间。
            self.pending.get_or_insert_with(Instant::now);
        } else {
            self.pending = None;
            self.full_speed = false;
        }
        Ok(())
    }

    pub fn commit(&mut self, actual_full_speed: bool) -> Result<()> {
        if self
            .pending
            .is_none_or(|at| at.elapsed() > Duration::from_secs(3))
            || !actual_full_speed
        {
            bail!("固件全速模式未完成确认");
        }
        self.pending = None;
        self.full_speed = true;
        Ok(())
    }

    pub fn recovery(
        &mut self,
        reason: Option<RecoveryReason>,
        actual_full_speed: bool,
    ) -> Option<String> {
        if self
            .pending
            .is_some_and(|at| at.elapsed() > Duration::from_secs(3))
        {
            return Some("FullSpeedActivationTimeout".into());
        }
        if self.full_speed && !actual_full_speed {
            self.full_speed = false;
        }
        match reason {
            Some(RecoveryReason::Overheat) if self.full_speed || self.pending.is_some() => None,
            other => other.map(|value| format!("{value:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_activation_expires_and_repeated_preparation_cannot_extend_it() {
        let mut policy = Policy::new(GuardPolicy::FirmwareFullSpeed);
        policy.pending = Some(Instant::now() - Duration::from_secs(4));
        policy
            .prepare(true, Some(&RecoveryReason::Overheat))
            .unwrap();
        assert!(policy.commit(true).is_err());
        assert_eq!(
            policy.recovery(None, true).as_deref(),
            Some("FullSpeedActivationTimeout")
        );
    }

    #[test]
    fn confirmed_full_speed_only_exempts_overheat_and_observed_mode_must_remain_full_speed() {
        let mut policy = Policy::new(GuardPolicy::FirmwareFullSpeed);
        assert!(policy.commit(false).is_err());
        policy.commit(true).unwrap();
        assert_eq!(policy.recovery(Some(RecoveryReason::Overheat), true), None);
        assert!(
            policy
                .prepare(false, Some(&RecoveryReason::Overheat))
                .is_err()
        );
        assert_eq!(policy.recovery(Some(RecoveryReason::Overheat), true), None);
        assert_eq!(
            policy
                .recovery(Some(RecoveryReason::SensorLost), true)
                .as_deref(),
            Some("SensorLost")
        );
        assert_eq!(
            policy
                .recovery(Some(RecoveryReason::Overheat), false)
                .as_deref(),
            Some("Overheat")
        );
    }

    #[test]
    fn protected_handoff_is_explicit_and_reversible_before_cooling_is_lowered() {
        let mut policy = Policy::new(GuardPolicy::Protected);
        policy.prepare(true, None).unwrap();
        policy.commit(true).unwrap();
        policy.prepare(false, None).unwrap();
        assert_eq!(
            policy
                .recovery(Some(RecoveryReason::Overheat), true)
                .as_deref(),
            Some("Overheat")
        );
    }
}
