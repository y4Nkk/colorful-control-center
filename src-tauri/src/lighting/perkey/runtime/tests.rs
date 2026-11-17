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
    tx.send(Envelope {
        epoch: 1,
        event: KeyEvent {
            scan: 21,
            down: true,
        },
    })
    .unwrap();
    assert_eq!(
        sent_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        [7, 255, 21, 0, 0]
    );
    control.stop.store(true, Ordering::SeqCst);
    drop(tx);
    handle.join().unwrap();
    assert!(sent_rx.try_recv().is_err());
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
        control.overflow.store(overflow, Ordering::SeqCst);
        let (tx, rx) = mpsc::sync_channel(64);
        tx.send(Envelope {
            epoch: 0,
            event: KeyEvent {
                scan: 20,
                down: true,
            },
        })
        .unwrap();
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
