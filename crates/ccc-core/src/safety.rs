//! 独立风扇监护的状态判断，不包含进程或硬件访问。
#[derive(Debug, PartialEq)]
pub enum RecoveryReason {
    Disconnected,
    HeartbeatTimeout,
    SensorLost,
    Overheat,
}

pub fn recovery_reason(
    connected: bool,
    age_ms: u64,
    cpu: Option<f64>,
    gpu: Option<f64>,
) -> Option<RecoveryReason> {
    if !connected {
        return Some(RecoveryReason::Disconnected);
    }
    if age_ms > 3000 {
        return Some(RecoveryReason::HeartbeatTimeout);
    }
    if cpu.is_none_or(|v| !v.is_finite() || !(0.0..=150.0).contains(&v))
        || gpu.is_some_and(|v| !v.is_finite() || !(0.0..=150.0).contains(&v))
    {
        return Some(RecoveryReason::SensorLost);
    }
    if cpu.is_some_and(|v| v >= 95.0) || gpu.is_some_and(|v| v >= 95.0) {
        return Some(RecoveryReason::Overheat);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn any_missing_heartbeat_or_invalid_sensor_recovers() {
        assert_eq!(
            recovery_reason(false, 0, Some(50.0), None),
            Some(RecoveryReason::Disconnected)
        );
        assert_eq!(
            recovery_reason(true, 3001, Some(50.0), None),
            Some(RecoveryReason::HeartbeatTimeout)
        );
        assert_eq!(
            recovery_reason(true, 0, None, None),
            Some(RecoveryReason::SensorLost)
        );
        assert_eq!(
            recovery_reason(true, 0, Some(f64::NAN), None),
            Some(RecoveryReason::SensorLost)
        );
        assert_eq!(
            recovery_reason(true, 0, Some(70.0), Some(95.0)),
            Some(RecoveryReason::Overheat)
        );
        assert_eq!(recovery_reason(true, 0, Some(50.0), Some(60.0)), None);
    }
}
