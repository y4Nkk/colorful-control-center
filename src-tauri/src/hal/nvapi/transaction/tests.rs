use super::*;
use std::{collections::VecDeque, sync::Mutex};
#[derive(Default)]
struct MockState {
    core_khz: i32,
    mem_khz: i32,
    core_editable: bool,
    mem_editable: bool,
    core_range_khz: (i32, i32),
    mem_range_khz: (i32, i32),
    write_errors: VecDeque<&'static str>,
    silent_reject: bool,
    clamp_to_range: bool,
    writes: Vec<(Option<i32>, Option<i32>)>,
}
impl MockState {
    fn editable() -> Self {
        Self {
            core_editable: true,
            mem_editable: true,
            core_range_khz: (-500_000, 900_000),
            mem_range_khz: (-1_500_000, 3_000_000),
            ..Default::default()
        }
    }
    fn parsed(&self) -> ParsedPstates {
        let domain = |editable, delta, range: (i32, i32)| DomainReading {
            present: true,
            editable,
            delta_khz: delta,
            min_khz: range.0,
            max_khz: range.1,
        };
        ParsedPstates {
            editable: true,
            p0_present: true,
            core: domain(self.core_editable, self.core_khz, self.core_range_khz),
            memory: domain(self.mem_editable, self.mem_khz, self.mem_range_khz),
            count_clamped: false,
        }
    }
    fn apply_write(&mut self, core: Option<i32>, mem: Option<i32>) {
        let clamp = |v: i32, r: (i32, i32)| {
            if self.clamp_to_range {
                v.clamp(r.0.max(-50_000), r.1.min(50_000))
            } else {
                v
            }
        };
        if let Some(v) = core {
            self.core_khz = clamp(v, self.core_range_khz);
        }
        if let Some(v) = mem {
            self.mem_khz = clamp(v, self.mem_range_khz);
        }
    }
}
struct MockIo(Mutex<MockState>);
impl MockIo {
    fn new() -> Self {
        Self(Mutex::new(MockState::editable()))
    }
    fn with<R>(&self, f: impl FnOnce(&mut MockState) -> R) -> R {
        f(&mut self.0.lock().unwrap())
    }
}
impl PstatesIo for MockIo {
    fn read(&self) -> Result<ParsedPstates> {
        Ok(self.0.lock().unwrap().parsed())
    }
    fn write(&self, core: Option<i32>, mem: Option<i32>) -> Result<()> {
        let mut io = self.0.lock().unwrap();
        io.writes.push((core, mem));
        if let Some(error) = io.write_errors.pop_front() {
            return Err(anyhow!(error));
        }
        if !io.silent_reject {
            io.apply_write(core, mem);
        }
        Ok(())
    }
}
const CORE: DomainCaps = DomainCaps {
    min_mhz: -300,
    max_mhz: 300,
};
const MEM: DomainCaps = DomainCaps {
    min_mhz: -1000,
    max_mhz: 1000,
};
fn run(io: &MockIo, core: Option<i32>, mem: Option<i32>) -> TransactionOutcome {
    run_transaction(io, CORE, MEM, core, mem)
}
#[test]
fn happy_path_confirms_and_reports_pre_state() {
    let io = MockIo::new();
    io.with(|m| m.core_khz = 135_000);
    let o = run(&io, Some(150), Some(500));
    assert!(o.ok, "{}", o.message);
    assert_eq!(o.confirmed_core_mhz, Some(150));
    assert_eq!(o.confirmed_mem_mhz, Some(500));
    assert_eq!(
        o.pre,
        RawOffsets {
            core_khz: Some(135_000),
            mem_khz: Some(0)
        }
    );
    io.with(|m| assert_eq!(m.writes.len(), 1));
}
#[test]
fn out_of_cap_and_invalid_requests_rejected_before_any_write() {
    let io = MockIo::new();
    for (core, mem, expected) in [
        (None, None, "未提供任何偏移"),
        (Some(301), None, "超出允许范围"),
        (Some(-301), None, "超出允许范围"),
        (None, Some(1001), "超出允许范围"),
        (None, Some(-1001), "超出允许范围"),
        (Some(2_000_000), None, "超出允许范围"),
    ] {
        let o = run(&io, core, mem);
        assert!(!o.ok);
        assert!(o.message.contains(expected));
        assert!(o.rollback_error.is_none());
    }
    assert!(mhz_to_khz(i32::MAX).is_err());
    io.with(|m| assert!(m.writes.is_empty()));
}
#[test]
fn readonly_domains_rejected_cleanly() {
    let io = MockIo::new();
    io.with(|m| m.mem_editable = false);
    let o = run(&io, Some(100), Some(100));
    assert!(!o.ok && o.message.contains("未开放显存"));
    io.with(|m| assert!(m.writes.is_empty()));
}
#[test]
fn ranges_intersect_driver_and_cap() {
    let io = MockIo::new();
    io.with(|m| m.core_range_khz = (-100_000, 250_000));
    let tight = run(&io, Some(260), None);
    assert!(!tight.ok && tight.message.contains("-100…+250"));
    assert!(run(&io, Some(100), None).ok);
    io.with(|m| m.core_range_khz = (-500_000, 900_000));
    assert!(run(&io, Some(-300), None).ok);
    assert!(!run(&io, Some(301), None).ok);
    io.with(|m| m.core_range_khz = (0, 0));
    assert!(!run(&io, Some(300), None).ok);
    let inverted = DomainReading {
        present: true,
        editable: true,
        delta_khz: 0,
        min_khz: 300_000,
        max_khz: -300_000,
    };
    assert_eq!(effective_range(&inverted, CORE), (1, 0));
}
#[test]
fn silent_reject_triggers_verified_rollback() {
    let io = MockIo::new();
    io.with(|m| {
        m.core_khz = 100_000;
        m.silent_reject = true;
    });
    let o = run(&io, Some(200), None);
    assert!(!o.ok && o.message.contains("写入未生效"));
    assert!(o.rollback_error.is_none());
    io.with(|m| assert_eq!(m.writes, vec![(Some(200_000), None), (Some(100_000), None)]));
}
#[test]
fn rollback_uses_raw_khz_avoiding_rounding_drift() {
    let io = MockIo::new();
    io.with(|m| {
        m.core_khz = 135_500;
        m.silent_reject = true;
    });
    assert!(!run(&io, Some(150), None).ok);
    io.with(|m| assert_eq!(m.writes.last(), Some(&(Some(135_500), None))));
}
#[test]
fn clamp_by_driver_rolls_back() {
    let io = MockIo::new();
    io.with(|m| {
        m.core_range_khz = (-500_000, 500_000);
        m.clamp_to_range = true;
    });
    let o = run(&io, Some(100), None);
    assert!(!o.ok && o.message.contains("写入未生效"));
    assert!(o.rollback_error.is_none());
    io.with(|m| assert_eq!(m.core_khz, 0));
}
#[test]
fn write_error_reports_rollback_failure_when_rollback_also_fails() {
    let io = MockIo::new();
    let write_error = "NVAPI_INVALID_USER_PRIVILEGE (-137)";
    let rollback_error = "NVAPI_ERROR (-1)";
    io.with(|m| {
        m.core_khz = 100_000;
        m.write_errors.extend([write_error, rollback_error]);
    });
    let o = run(&io, Some(200), None);
    assert!(!o.ok);
    assert!(
        o.message
            .contains(&format!("写入 GPU 频率偏移失败: {write_error}"))
    );
    let rollback = o.rollback_error.unwrap();
    assert!(rollback.contains("回滚失败") && rollback.contains(rollback_error));
    assert!(!rollback.contains(write_error));
}
