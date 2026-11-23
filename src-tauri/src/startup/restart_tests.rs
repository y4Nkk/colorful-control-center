use super::*;

#[test]
fn restart_arguments_are_strict_and_never_include_login_startup() {
    let binding = Binding {
        token: "a".repeat(32),
        parent_id: 123,
    };
    for (kind, name) in [
        (RestartMode::Interface, "interface"),
        (RestartMode::Administrator, "administrator"),
    ] {
        let mut args = kind.arguments(&binding, "S-1-5-12".into()).to_vec();
        assert_eq!(args, ["--restart", name, "123", &binding.token, "S-1-5-12"]);
        args.push("--autostart".into());
        assert!(receive(&args).is_err());
    }
}

#[test]
fn exit_takes_priority_even_when_ready_was_signaled() {
    let ready = Event::anonymous().unwrap();
    let exited = Event::anonymous().unwrap();
    assert!(
        wait_ready(exited.raw(), &ready, 0)
            .unwrap_err()
            .to_string()
            .contains("超时")
    );
    ready.signal().unwrap();
    wait_ready(exited.raw(), &ready, 0).unwrap();
    exited.signal().unwrap();
    assert!(
        wait_ready(exited.raw(), &ready, 0)
            .unwrap_err()
            .to_string()
            .contains("退出")
    );
}

#[test]
fn child_requires_a_commit_and_cancellation_takes_priority() {
    let binding = Binding::new().unwrap();
    let sender = HandoffEvents::bind(&binding, Event::create).unwrap();
    let receiver = HandoffEvents::bind(&binding, Event::open).unwrap();
    let parent = Event::anonymous().unwrap();
    parent.signal().unwrap();
    assert!(!receiver.await_decision(&parent).unwrap());
    sender.commit.signal().unwrap();
    assert!(receiver.await_decision(&parent).unwrap());
    sender.cancel.signal().unwrap();
    assert!(!receiver.await_decision(&parent).unwrap());
}

#[test]
fn restart_waits_for_all_lease_holders_before_taking_control() {
    let binding = Binding::new().unwrap();
    let name = format!("Local\\CCC.test-restart.{}", binding.token);
    let old = ControllerLease::try_acquire(&name).unwrap().unwrap();
    let recovery = ControllerLease::retain(&name).unwrap();
    drop(old);
    let blocked = wait_for_lease(&name, Duration::from_millis(20));
    assert!(blocked.err().unwrap().to_string().contains("未接管硬件"));
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(60));
        drop(recovery);
    });
    let next = wait_for_lease(&name, Duration::from_secs(2)).unwrap();
    assert!(ControllerLease::try_acquire(&name).unwrap().is_none());
    drop(next);
    release.join().unwrap();
    assert!(ControllerLease::try_acquire(&name).unwrap().is_some());
}
