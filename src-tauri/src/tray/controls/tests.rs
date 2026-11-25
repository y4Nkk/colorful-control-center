use super::*;
use crate::hal::oem_dchu::Dchu;
use ccc_core::fan::oem_fan_mode_bits;
use std::cell::{Cell, RefCell};

thread_local! {
    static PAGES: RefCell<[[u8; 256]; 8]> = const { RefCell::new([[0; 256]; 8]) };
    static FAN_MODE: Cell<u8> = const { Cell::new(FAN_MODE_AUTO) };
    static WRITES: Cell<usize> = const { Cell::new(0) };
}

unsafe extern "system" fn read_packet(command: i32, output: *mut u8) -> i32 {
    unsafe {
        std::ptr::write_bytes(output, 0, 257);
        output.add(12).write(2);
        if command == 13 {
            output
                .add(14)
                .write(oem_fan_mode_bits(FAN_MODE.get()).unwrap_or_default());
        }
    }
    command
}

unsafe extern "system" fn read_settings(
    page: i32,
    offset: i32,
    length: i32,
    output: *mut u8,
) -> i32 {
    PAGES.with(|pages| unsafe {
        std::ptr::copy_nonoverlapping(
            pages.borrow()[page as usize].as_ptr().add(offset as usize),
            output,
            length as usize,
        );
    });
    4096
}

unsafe extern "system" fn send(command: i32, input: *const u8, length: i32) -> i32 {
    WRITES.with(|writes| writes.set(writes.get() + 1));
    if command == 121 && length == 4 {
        let payload = unsafe { std::slice::from_raw_parts(input, 4) };
        if let [mode, 0, 0, 1] = payload {
            FAN_MODE.set(*mode);
        }
    }
    command
}

unsafe extern "system" fn save(page: i32, offset: i32, length: i32, input: *const u8) -> i32 {
    WRITES.with(|writes| writes.set(writes.get() + 1));
    let start = offset as usize;
    PAGES.with(|pages| {
        pages.borrow_mut()[page as usize][start..start + length as usize]
            .copy_from_slice(unsafe { std::slice::from_raw_parts(input, length as usize) })
    });
    1
}

unsafe extern "system" fn capabilities(command: i32, _: *const u8, _: i32, output: *mut u8) -> i32 {
    unsafe {
        std::ptr::write_bytes(output, 0, 257);
        output.write(1);
        output.add(256).write(1);
    }
    command
}

#[test]
fn full_speed_cancels_without_writes_and_rejects_a_changed_confirmation_context() {
    PAGES.with(|pages| *pages.borrow_mut() = [[0; 256]; 8]);
    FAN_MODE.set(FAN_MODE_AUTO);
    WRITES.set(0);
    let hal = Hal::test_instance(
        Dchu::test_io(read_packet, read_settings, send, save).with_test_exchange(capabilities),
        None,
    );
    assert_eq!(
        switch_fan(&hal, FanAction::FullSpeed, |title, detail| {
            assert_eq!(title, "确认风扇全速");
            assert!(detail.contains("在托盘选择「自动」"));
            Ok(false)
        })
        .unwrap(),
        None
    );
    assert_eq!(WRITES.get(), 0);

    let error = switch_fan(&hal, FanAction::FullSpeed, |_, _| {
        let _gate = hal.control_gate();
        let _guard = _gate.try_lock().expect("等待确认期间不持有 HAL 写锁");
        PAGES.with(|pages| pages.borrow_mut()[1][1] = 2);
        Ok(true)
    })
    .unwrap_err();
    assert!(error.to_string().contains("状态已变化"));
    assert_eq!(WRITES.get(), 0);

    let result = switch_fan(&hal, FanAction::FullSpeed, |_, _| Ok(true)).unwrap();
    assert_eq!(result, Some(FAN_MODE_FULL_SPEED));
    let controls = Controls {
        presets: Err("无预设".into()),
        fans: Ok(read_fans(&hal, None).unwrap()),
    };
    assert_eq!(
        verify(
            Control::Fan(FanAction::FullSpeed),
            result.unwrap(),
            &controls
        )
        .unwrap(),
        "全速"
    );
    assert!(verify(Control::Fan(FanAction::Auto), FAN_MODE_AUTO, &controls).is_err());

    let result = switch_fan(&hal, FanAction::Auto, |_, _| panic!("自动档应直接提交")).unwrap();
    assert_eq!(result, Some(FAN_MODE_AUTO));
    let fans = read_fans(&hal, None).unwrap();
    assert_eq!(fans.active, Some(FanAction::Auto));
    assert_eq!(fans.label, "自动");
}
