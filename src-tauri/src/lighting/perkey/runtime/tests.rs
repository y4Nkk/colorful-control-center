use super::*;

fn config() -> (Values, DeviceInfo) {
    (
        Values {
            enabled: true,
            brightness: 3,
            speed: 1,
            pattern: super::super::Pattern::Ripple {
                reactive: false,
                color: None,
            },
        },
        super::super::device::decode([0; 3], [1, 0, 0]),
    )
}
#[test]
fn prepared_inactive_session_and_stale_epochs_cannot_output() {
    let control = Arc::new(Control::default());
    let gate = Arc::new(Mutex::new(()));
    let (values, info) = config();
    let (tx, rx) = mpsc::sync_channel(64);
    let (sent_tx, sent_rx) = mpsc::channel();
    let shared = control.clone();
    let handle = thread::spawn(move || {
        run(rx, &shared, &gate, &values, &info, |p| {
            sent_tx.send(p).unwrap();
            Ok(())
        })
    });
    control.pause();
    control.active.store(true, Ordering::SeqCst);
    tx.send(Envelope {
        epoch: 0,
        event: KeyEvent {
            scan: 20,
            down: true,
        },
    })
    .unwrap();
    control.enqueue(
        &tx,
        KeyEvent {
            scan: 21,
            down: true,
        },
    );
    assert_eq!(
        sent_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        [7, 255, 21, 0, 0]
    );
    control.shutdown();
    assert_eq!(
        sent_rx.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    );
    handle.join().unwrap();
}

#[test]
fn win_deadline_and_shutdown_wake_output_without_further_input() {
    let (values, info) = config();
    let control = Arc::new(Control::default());
    control.active.store(true, Ordering::SeqCst);
    let shared = control.clone();
    let (tx, rx) = mpsc::sync_channel(64);
    let (sent_tx, sent_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        run(rx, &shared, &Mutex::new(()), &values, &info, |p| {
            sent_tx.send(p).unwrap();
            Ok(())
        })
    });

    let started = Instant::now();
    control.enqueue(
        &tx,
        KeyEvent {
            scan: 0xE05B,
            down: true,
        },
    );
    assert_eq!(
        sent_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        [7, 255, 0xDB, 0, 0]
    );
    assert!(started.elapsed() >= Duration::from_millis(50));
    control.shutdown();
    assert_eq!(
        sent_rx.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    );
    handle.join().unwrap();
    drop(tx);
}
#[test]
fn pause_cancels_pending_output_while_another_control_holds_gate() {
    let control = Arc::new(Control::default());
    control.active.store(true, Ordering::SeqCst);
    let gate = Arc::new(Mutex::new(()));
    let guard = gate.lock().unwrap();
    let shared = control.clone();
    let worker_gate = gate.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        entered_tx.send(()).unwrap();
        output(
            &shared,
            &worker_gate,
            0,
            vec![[7, 255, 10, 0, 0]],
            &mut |_| panic!("暂停后的旧事件不可写入"),
        )
        .unwrap();
    });
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    control.pause();
    drop(guard);
    handle.join().unwrap();
}
#[test]
fn overflow_and_transport_failure_stop_instead_of_losing_key_release() {
    let (values, info) = config();
    for overflow in [false, true] {
        let control = Control::default();
        control.active.store(true, Ordering::SeqCst);
        let (tx, rx) = mpsc::sync_channel(1);
        control.enqueue(
            &tx,
            KeyEvent {
                scan: 20,
                down: true,
            },
        );
        if overflow {
            control.enqueue(
                &tx,
                KeyEvent {
                    scan: 20,
                    down: false,
                },
            );
        }
        run(rx, &control, &Mutex::new(()), &values, &info, |_| {
            bail!("isolated HID failure")
        });
        assert!(control.stop.load(Ordering::SeqCst));
        assert!(!control.accepts(0));
        let error = control.error.lock().unwrap().clone().unwrap();
        assert!(error.contains(if overflow {
            "队列已满"
        } else {
            "isolated HID failure"
        }));
    }
}
