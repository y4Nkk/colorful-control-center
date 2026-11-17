//! 真实 Windows 对象/进程，名称和文件逐次隔离；不运行应用或访问硬件。
#[path = "support/lease.rs"]
mod support;
use support::Fixture;

#[test]
fn concurrent_main_processes_have_exactly_one_controller() {
    let mut f = Fixture::new();
    f.spawn("a", "owner");
    f.spawn("b", "owner");
    let a = f.wait("a.result");
    let b = f.wait("b.result");
    assert!(matches!(
        (a.as_str(), b.as_str()),
        ("ACQUIRED", "BUSY") | ("BUSY", "ACQUIRED")
    ));
    f.signal("release-parent");
    f.finish("a", true);
    f.finish("b", true);
}

#[test]
fn normal_exit_releases_control_for_a_later_main_process() {
    let mut f = Fixture::new();
    f.spawn("owner", "owner");
    assert_eq!(f.wait("owner.result"), "ACQUIRED");
    f.spawn("duplicate", "probe");
    assert_eq!(f.wait("duplicate.result"), "BUSY");
    f.finish("duplicate", true);
    f.signal("release-parent");
    f.finish("owner", true);
    f.spawn("next", "probe");
    assert_eq!(f.wait("next.result"), "ACQUIRED");
    f.finish("next", true);
}

#[test]
fn killed_main_without_watchdog_leaves_no_stale_lock() {
    let mut f = Fixture::new();
    f.spawn("owner", "owner");
    assert_eq!(f.wait("owner.result"), "ACQUIRED");
    f.kill("owner");
    f.spawn("next", "probe");
    assert_eq!(f.wait("next.result"), "ACQUIRED");
    f.finish("next", true);
}

#[test]
fn watchdog_keeps_control_until_recovery_after_parent_is_killed() {
    let mut f = Fixture::new();
    f.spawn("parent", "watchdog");
    assert_eq!(f.wait("parent.ready"), "ARMED");
    f.retain_guard_handle("parent");
    f.kill("parent");
    f.wait("parent.recovery-started");
    f.spawn("too-early", "probe");
    assert_eq!(f.wait("too-early.result"), "BUSY");
    f.finish("too-early", true);
    f.signal("release-recovery");
    assert_eq!(f.wait("parent.restored"), "1");
    f.wait_guard_exit();
    f.spawn("next", "probe");
    assert_eq!(f.wait("next.result"), "ACQUIRED");
    f.finish("next", true);
}

#[test]
fn watchdog_death_does_not_release_a_living_main_process() {
    let mut f = Fixture::new();
    f.spawn("parent", "watchdog");
    f.wait("parent.ready");
    f.retain_guard_handle("parent");
    f.kill_guard();
    f.wait("parent.watchdog-error");
    f.spawn("duplicate", "probe");
    assert_eq!(f.wait("duplicate.result"), "BUSY");
    f.finish("duplicate", true);
    f.signal("release-parent");
    f.finish("parent", true);
    f.spawn("next", "probe");
    assert_eq!(f.wait("next.result"), "ACQUIRED");
    f.finish("next", true);
}

#[test]
fn explicit_disarm_and_normal_exit_leave_no_recovery_or_lock() {
    let mut f = Fixture::new();
    f.spawn("parent", "watchdog");
    f.wait("parent.ready");
    f.retain_guard_handle("parent");
    f.signal("release-parent");
    f.finish("parent", true);
    f.wait_guard_exit();
    assert!(!f.exists("parent.recovery-started") && !f.exists("parent.restored"));
    f.spawn("next", "probe");
    assert_eq!(f.wait("next.result"), "ACQUIRED");
    f.finish("next", true);
}

#[test]
fn orphan_watchdog_cannot_initialize_or_arm_without_main_control() {
    let mut f = Fixture::new();
    f.spawn("orphan", "orphan");
    assert!(f.wait("orphan.child.exit").contains("主控制权不存在"));
    f.finish("orphan", false);
    assert!(!f.exists("orphan.recovery-started"));
    f.spawn("next", "probe");
    assert_eq!(f.wait("next.result"), "ACQUIRED");
    f.finish("next", true);
}
