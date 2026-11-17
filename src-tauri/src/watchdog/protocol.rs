//! 监护状态机与管道消息，不持有硬件；恢复操作由调用者提供。
use anyhow::{Result, anyhow, bail};
use ccc_core::safety::{RecoveryReason, recovery_reason};
use std::{
    io::{BufRead, BufReader, Write},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
#[path = "policy.rs"]
mod policy;
pub use policy::GuardPolicy;
use policy::Policy;

pub fn read_lines(input: impl std::io::Read + Send + 'static) -> Receiver<String> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(input).lines() {
            match line {
                Ok(line) => {
                    if send.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    receive
}

fn output(value: &str) {
    let mut stdout = std::io::stdout().lock();
    // 父进程已被强杀时输出管道会关闭；报告失败不能阻断已经完成的恢复。
    let _ = writeln!(stdout, "{value}");
    let _ = stdout.flush();
}

pub fn report_failure(message: &str) {
    output(&format!("FAILED:{}", message.replace(['\r', '\n'], " ")));
}

pub fn run(
    input: Receiver<String>,
    mut sample: impl FnMut() -> Option<(f64, Option<f64>)>,
    restore: impl Fn() -> Result<()>,
    initial_policy: GuardPolicy,
    mut full_speed: impl FnMut() -> bool,
) -> Result<()> {
    let initial = sample().ok_or_else(|| anyhow!("传感器无效，未启用手动控制"))?;
    let mut gpu_required = initial.1.is_some();
    let Some(mut policy) = initialize(&input, initial, initial_policy, full_speed())? else {
        return Ok(());
    };
    let mut pulse = Instant::now();
    loop {
        let message = input.recv_timeout(Duration::from_millis(300));
        match message.as_deref() {
            Ok("DISARM") => {
                output("DISARMED");
                return Ok(());
            }
            Ok("HB") => {
                pulse = Instant::now();
            }
            _ => {}
        }
        let connected = !matches!(message, Err(mpsc::RecvTimeoutError::Disconnected));
        let temperatures = sample();
        gpu_required |= temperatures.and_then(|s| s.1).is_some();
        let reason = recovery_reason(
            connected,
            pulse.elapsed().as_millis() as u64,
            temperatures
                .filter(|s| !gpu_required || s.1.is_some())
                .map(|s| s.0),
            temperatures.and_then(|s| s.1),
        );
        if reason
            .as_ref()
            .is_some_and(|value| !matches!(value, RecoveryReason::Overheat))
        {
            return recover(&restore, &format!("{:?}", reason.unwrap()));
        }
        let actual_full_speed = full_speed();
        policy_message(
            &mut policy,
            message.as_deref().ok(),
            reason.as_ref(),
            actual_full_speed,
        );
        if let Some(reason) = policy.recovery(reason, actual_full_speed) {
            return recover(&restore, &reason);
        }
    }
}

fn initialize(
    input: &Receiver<String>,
    initial: (f64, Option<f64>),
    initial_policy: GuardPolicy,
    actual_full_speed: bool,
) -> Result<Option<Policy>> {
    let mut policy = Policy::new(initial_policy);
    if let Some(reason) = policy.recovery(
        recovery_reason(true, 0, Some(initial.0), initial.1),
        actual_full_speed,
    ) {
        match reason.as_str() {
            "Overheat" => bail!("温度达到95°C保护阈值，保留自动散热，未启用手动控制"),
            _ => bail!("风扇传感器异常，保留自动散热，未启用手动控制"),
        }
    }
    output("READY");
    if input.recv_timeout(Duration::from_secs(3)).ok().as_deref() != Some("ARM") {
        return Ok(None);
    }
    output("ARMED");
    Ok(Some(policy))
}

fn policy_message(
    policy: &mut Policy,
    message: Option<&str>,
    reason: Option<&RecoveryReason>,
    actual: bool,
) {
    let result = match message {
        Some("PREPARE_FULL_SPEED" | "PREPARE_AUTO") => policy.prepare(true, reason),
        Some("PREPARE_PROTECTED") => policy.prepare(false, reason),
        Some("COMMIT_FULL_SPEED") => policy.commit(actual),
        _ => return,
    };
    match result {
        Ok(()) => output("POLICY_READY"),
        Err(error) => output(&format!("REJECTED:{error:#}")),
    }
}

fn recover(restore: &impl Fn() -> Result<()>, reason: &str) -> Result<()> {
    for attempt in 0..3 {
        match restore() {
            Ok(()) => {
                output(&format!("RESTORED:{reason}"));
                return Ok(());
            }
            Err(error) if attempt == 2 => {
                output(&format!("RECOVERY_FAILED:{error:#}"));
                return Err(error);
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn armed_disconnect_restores_once() {
        use std::cell::Cell;
        let (send, receive) = mpsc::channel();
        send.send("ARM".into()).unwrap();
        drop(send);
        let restored = Cell::new(0);
        run(
            receive,
            || Some((50.0, Some(45.0))),
            || {
                restored.set(restored.get() + 1);
                Ok(())
            },
            GuardPolicy::Protected,
            || false,
        )
        .unwrap();
        assert_eq!(restored.get(), 1);
    }
    #[test]
    fn unarmed_disconnect_never_writes() {
        let (send, receive) = mpsc::channel();
        drop(send);
        run(
            receive,
            || Some((50.0, None)),
            || panic!("未 ARM 不应写入"),
            GuardPolicy::Protected,
            || false,
        )
        .unwrap();
    }
    #[test]
    fn overheat_recovers_without_waiting_for_timeout() {
        let (send, receive) = mpsc::channel();
        send.send("ARM".into()).unwrap();
        send.send("HB".into()).unwrap();
        let count = std::cell::Cell::new(0);
        run(
            receive,
            || {
                count.set(count.get() + 1);
                Some((if count.get() == 1 { 50.0 } else { 96.0 }, None))
            },
            || Ok(()),
            GuardPolicy::Protected,
            || false,
        )
        .unwrap();
    }

    #[test]
    fn explicit_disarm_never_restores_again() {
        let (send, receive) = mpsc::channel();
        send.send("ARM".into()).unwrap();
        send.send("DISARM".into()).unwrap();
        run(
            receive,
            || Some((50.0, None)),
            || panic!("已经解除监护"),
            GuardPolicy::Protected,
            || false,
        )
        .unwrap();
    }

    #[test]
    fn loss_of_previously_present_gpu_triggers_recovery() {
        let (send, receive) = mpsc::channel();
        send.send("ARM".into()).unwrap();
        send.send("HB".into()).unwrap();
        let first = std::cell::Cell::new(true);
        let restored = std::cell::Cell::new(false);
        run(
            receive,
            || Some((50.0, first.replace(false).then_some(45.0))),
            || {
                restored.set(true);
                Ok(())
            },
            GuardPolicy::Protected,
            || false,
        )
        .unwrap();
        assert!(restored.get());
    }

    #[test]
    fn gpu_seen_after_arming_must_not_disappear_silently() {
        let (send, receive) = mpsc::channel();
        for message in ["ARM", "HB", "HB"] {
            send.send(message.into()).unwrap();
        }
        let calls = std::cell::Cell::new(0);
        let restored = std::cell::Cell::new(false);
        run(
            receive,
            || {
                let index = calls.get();
                calls.set(index + 1);
                Some((50.0, (index == 1).then_some(45.0)))
            },
            || {
                restored.set(true);
                Ok(())
            },
            GuardPolicy::Protected,
            || false,
        )
        .unwrap();
        assert!(restored.get());
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn sensor_loss_retries_restoration_and_reports_failure() {
        let (send, receive) = mpsc::channel();
        send.send("ARM".into()).unwrap();
        send.send("HB".into()).unwrap();
        let first = std::cell::Cell::new(true);
        let attempts = std::cell::Cell::new(0);
        let result = run(
            receive,
            || first.replace(false).then_some((50.0, None)),
            || {
                attempts.set(attempts.get() + 1);
                bail!("模拟驱动拒绝恢复")
            },
            GuardPolicy::Protected,
            || false,
        );
        assert_eq!(attempts.get(), 3);
        assert!(result.unwrap_err().to_string().contains("拒绝恢复"));
    }

    #[test]
    fn heartbeat_timeout_recovers_even_with_open_pipe() {
        let (send, receive) = mpsc::channel();
        send.send("ARM".into()).unwrap();
        let restored = std::cell::Cell::new(false);
        run(
            receive,
            || Some((50.0, None)),
            || {
                restored.set(true);
                Ok(())
            },
            GuardPolicy::Protected,
            || false,
        )
        .unwrap();
        assert!(restored.get());
    }

    #[test]
    fn initial_overheat_never_arms_or_writes() {
        let (_send, receive) = mpsc::channel();
        assert!(
            run(
                receive,
                || Some((96.0, None)),
                || panic!("未 ARM 不应写入"),
                GuardPolicy::Protected,
                || false
            )
            .is_err()
        );
    }
}
