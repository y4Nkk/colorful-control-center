use super::*;
use crate::display::test_fixture::Fixture;

#[test]
fn busy_query_does_not_wait_for_an_application_transaction_lock_or_write_displays() {
    let controller = Controller::default();
    let display = Fixture::new();
    assert!(!controller.is_busy(&display.controller).unwrap());
    let guard = controller.lock().unwrap();
    assert!(controller.is_busy(&display.controller).unwrap());
    drop(guard);
    assert!(!controller.is_busy(&display.controller).unwrap());
    assert!(display.io.0.lock().unwrap().writes.is_empty());
}
#[test]
fn a_poisoned_application_lock_is_an_error_instead_of_a_ready_state() {
    let controller = Arc::new(Controller::default());
    let writer = controller.clone();
    let failed = std::thread::spawn(move || {
        let _guard = writer.lock().unwrap();
        panic!("隔离档案事务失败");
    });
    assert!(failed.join().is_err());
    let display = Fixture::new();
    assert!(controller.is_busy(&display.controller).is_err());
    assert!(display.io.0.lock().unwrap().writes.is_empty());
}
