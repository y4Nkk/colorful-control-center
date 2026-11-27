use super::{BatteryAction, ERROR_MORE_DATA, read_friendly_name};

#[test]
fn battery_action_identifiers_match_the_oem_powerlife_binary() {
    let low = BatteryAction::Low.guid();
    assert_eq!(
        (low.data1, low.data2, low.data3, low.data4),
        (
            0xd8742dcb,
            0x3e6a,
            0x4b3c,
            [0xb3, 0xfe, 0x37, 0x46, 0x23, 0xcd, 0xcf, 0x06]
        )
    );
    let critical = BatteryAction::Critical.guid();
    assert_eq!(
        (
            critical.data1,
            critical.data2,
            critical.data3,
            critical.data4
        ),
        (
            0x637ea02f,
            0xbbcb,
            0x4015,
            [0x8e, 0x2c, 0xa1, 0xc7, 0xb9, 0xc0, 0xb5, 0x46]
        )
    );
}

#[test]
fn friendly_name_resizes_after_a_rename_and_decodes_utf16() {
    let name: Vec<u16> = "工作计划 🪟\0".encode_utf16().collect();
    let bytes = (name.len() * 2) as u32;
    let mut attempts = 0;
    let actual = read_friendly_name(|buffer, size| {
        attempts += 1;
        if buffer.is_null() {
            *size = 4;
            return 0;
        }
        if *size < bytes {
            *size = bytes;
            return ERROR_MORE_DATA;
        }
        unsafe {
            buffer
                .cast::<u16>()
                .copy_from_nonoverlapping(name.as_ptr(), name.len())
        };
        *size = bytes;
        0
    })
    .unwrap();
    assert_eq!(actual, "工作计划 🪟");
    assert_eq!(attempts, 3);
}

#[test]
fn invalid_friendly_name_buffer_size_is_rejected() {
    let result = read_friendly_name(|buffer, size| {
        assert!(buffer.is_null());
        *size = 3;
        0
    });
    assert!(result.unwrap_err().to_string().contains("长度无效"));
}
