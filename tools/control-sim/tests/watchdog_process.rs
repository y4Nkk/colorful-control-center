//! 真实 Windows 进程/管道，恢复出口为隔离日志；禁止加载应用或真实硬件。
mod support;
use support::Fixture;

#[test]
fn gui_worker_reports_high_temperature_without_a_timeout_or_manual_output() {
    let mut fixture = Fixture::start("initial-hot");
    let error = fixture.wait_file("parent.result");
    assert!(
        error.contains("95°C") && error.contains("未启用软件风扇曲线"),
        "{error}"
    );
    assert!(!error.contains("timed out"), "{error}");
    fixture.wait_parent();
    fixture.wait_child_exit();
    assert!(!fixture.exists("parent.ready") && !fixture.exists("restore.log"));
}

#[test]
fn handshake_and_disarm_leave_no_recovery_or_child() {
    let mut fixture = Fixture::start("disarm");
    fixture.wait_file("parent.ready");
    assert_eq!(fixture.wait_file("parent.result"), "DISARMED");
    fixture.wait_parent();
    fixture.wait_child_exit();
    assert!(!fixture.exists("restore.log"));
}

#[test]
fn disarm_requires_child_readback_and_retries_if_the_fan_is_still_manual() {
    let mut fixture = Fixture::start("disarm-unverified");
    fixture.wait_file("parent.ready");
    let result = fixture.wait_file("parent.result");
    assert!(result.contains("RECOVERY_FAILED") && result.contains("风扇归还未证实"));
    fixture.wait_parent();
    fixture.wait_child_exit();
    assert_eq!(fixture.read("restore.log").lines().count(), 3);
    assert!(fixture.read("child.exit").contains("Err"));
}

#[test]
fn forcibly_killed_parent_leaves_child_to_restore_once() {
    let mut fixture = Fixture::start("killed-parent");
    fixture.wait_file("parent.ready");
    fixture.kill_parent();
    fixture.wait_file("restore.log");
    fixture.wait_child_exit();
    assert_eq!(fixture.read("restore.log").lines().count(), 1);
    assert!(fixture.read("child.exit").contains("Ok"));
}

#[test]
fn open_pipe_without_heartbeats_recovers() {
    let mut fixture = Fixture::start("timeout");
    fixture.wait_file("parent.ready");
    let result = fixture.wait_file("parent.result");
    assert!(
        result.contains("HeartbeatTimeout")
            || result.contains("退出")
            || result.contains("连接中断")
    );
    fixture.wait_parent();
    fixture.wait_child_exit();
    assert_eq!(fixture.read("restore.log").lines().count(), 1);
}

#[test]
fn exited_watchdog_is_detected_by_parent() {
    let mut fixture = Fixture::start("child-exit");
    fixture.wait_file("parent.result");
    fixture.wait_parent();
    fixture.wait_child_exit();
    assert!(!fixture.exists("restore.log"));
}

#[test]
fn lost_output_is_failure_while_child_is_alive() {
    let mut fixture = Fixture::start("lost-output");
    let result = fixture.wait_file("parent.result");
    assert!(result.contains("输出连接中断"), "{result}");
    assert!(fixture.child_alive(), "回报管道关闭时应仍有活动子进程");
    fixture.release_output();
    fixture.wait_parent();
    fixture.wait_child_exit();
    assert_eq!(fixture.read("restore.log").lines().count(), 1);
    assert!(fixture.read("child.exit").contains("Ok"));
}

#[test]
fn sensor_loss_and_failed_recovery_cross_real_pipes() {
    for (scenario, count) in [
        ("sensor-loss", 1),
        ("late-gpu-loss", 1),
        ("recovery-failure", 3),
    ] {
        let mut fixture = Fixture::start(scenario);
        let message = fixture.wait_file("parent.result");
        if count == 3 {
            assert!(message.contains("RECOVERY_FAILED"), "{message}");
        } else {
            assert!(message.contains("SensorLost"), "{message}");
        }
        fixture.wait_parent();
        fixture.wait_child_exit();
        assert_eq!(fixture.read("restore.log").lines().count(), count);
        assert_eq!(fixture.read("child.exit").contains("Ok"), count == 1);
    }
}

#[test]
fn invalid_initial_sensor_never_arms_or_restores() {
    let mut fixture = Fixture::start("initial-invalid");
    fixture.wait_file("parent.result");
    fixture.wait_parent();
    fixture.wait_child_exit();
    assert!(!fixture.exists("parent.ready") && !fixture.exists("restore.log"));
}

#[test]
fn readiness_timeout_terminates_unarmed_child() {
    let mut fixture = Fixture::start("ready-timeout");
    fixture.wait_file("parent.result");
    fixture.wait_parent();
    assert!(!fixture.child_alive(), "未 READY 的进程应已结束并回收");
    assert!(!fixture.exists("parent.ready") && !fixture.exists("restore.log"));
}
