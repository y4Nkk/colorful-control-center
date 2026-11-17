use super::{Input, MESSAGE_LIMIT, Output, Plan};
use super::{matches, restore};
use crate::display::Access;
use anyhow::{Result, anyhow};
use std::{
    io::{BufRead, BufReader, Read},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

pub fn read(reader: impl Read + Send + 'static) -> Receiver<Input> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut line = Vec::new();
            let result = reader
                .by_ref()
                .take((MESSAGE_LIMIT + 1) as u64)
                .read_until(b'\n', &mut line);
            if result.is_err() || line.is_empty() || line.len() > MESSAGE_LIMIT {
                break;
            }
            let Ok(message) = serde_json::from_slice::<Input>(&line) else {
                break;
            };
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    receiver
}
pub fn run(input: Receiver<Input>, io: &dyn Access, mut output: impl FnMut(Output)) -> Result<()> {
    let plan = match input.recv_timeout(Duration::from_secs(3)) {
        Ok(Input::Prepare { plan }) => plan,
        _ => return Err(anyhow!("显示恢复计划未就绪")),
    };
    plan.validate()?;
    output(Output::Ready);
    if !matches!(input.recv_timeout(Duration::from_secs(3)), Ok(Input::Arm)) {
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_millis(plan.timeout_ms);
    output(Output::Armed);
    if wait_for_review(&input, io, &plan, deadline, &mut output) {
        return Ok(());
    }
    loop {
        match restore(io, &plan.changes) {
            Ok(()) => {
                output(Output::Recovered);
                return Ok(());
            }
            Err(error) => output(Output::RecoveryFailed {
                message: format!("{error:#}"),
            }),
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn wait_for_review(
    input: &Receiver<Input>,
    io: &dyn Access,
    plan: &Plan,
    deadline: Instant,
    output: &mut impl FnMut(Output),
) -> bool {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match input.recv_timeout(remaining.min(Duration::from_millis(250))) {
            Ok(Input::Keep { expected }) if expected == plan.targets() => {
                match matches(io, &expected) {
                    Ok(true) if Instant::now() < deadline => {
                        output(Output::Kept);
                        return true;
                    }
                    Err(error) => {
                        output(Output::Rejected {
                            message: format!("保留前回读失败：{error:#}"),
                        });
                        break;
                    }
                    _ => {}
                }
                output(Output::Rejected {
                    message: "实际显示模式不符合保留目标或试用已过期".into(),
                });
            }
            Ok(Input::Restored { expected }) if expected == plan.before() => {
                match matches(io, &expected) {
                    Ok(true) => {
                        output(Output::Released);
                        return true;
                    }
                    Err(error) => {
                        output(Output::Rejected {
                            message: format!("解除前回读失败：{error:#}"),
                        });
                        break;
                    }
                    _ => {}
                }
                output(Output::Rejected {
                    message: "尚未回到原显示模式".into(),
                });
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(_) => output(Output::Rejected {
                message: "显示监护请求与本次试用不一致".into(),
            }),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    false
}
