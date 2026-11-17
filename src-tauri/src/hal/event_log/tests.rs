use super::*;
fn record(number: u32, source: &str, messages: &[&str]) -> Vec<u8> {
    let mut raw = vec![0; 56];
    for value in [source, "fixture"] {
        raw.extend(value.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    }
    let offset = raw.len();
    for value in messages {
        raw.extend(value.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    }
    while raw.len() % 4 != 0 {
        raw.push(0);
    }
    raw.extend([0; 4]);
    let length = raw.len() as u32;
    for (at, value) in [
        (0, length),
        (4, 0x654c664c),
        (8, number),
        (20, 0x40000102),
        (36, offset as u32),
    ] {
        raw[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    raw[24..26].copy_from_slice(&4u16.to_le_bytes());
    raw[26..28].copy_from_slice(&(messages.len() as u16).to_le_bytes());
    let end = raw.len();
    raw[end - 4..].copy_from_slice(&length.to_le_bytes());
    raw
}
#[test]
fn zero_string_driver_events_and_unicode_oem_messages_share_one_decoder() {
    let raw = [
        record(5, "HKKbdFltr", &[]),
        record(6, "PowerBIOSServer_Out", &["Arg001^85", "中文🌈"]),
    ]
    .concat();
    let (next, events) = parse(&raw, 5).unwrap();
    assert_eq!(next, 7);
    assert_eq!(events[0].source, "HKKbdFltr");
    assert_eq!(events[0].event_id, 0x40000102);
    assert_eq!(events[0].event_type, 4);
    assert!(events[0].strings.is_empty());
    assert_eq!(events[1].strings[1], "中文🌈");
}
#[test]
fn rejects_bad_lengths_markers_sequence_and_overlapping_string_offsets() {
    for kind in 0..7 {
        let mut raw = record(1, "HKKbdFltr", &["text"]);
        match kind {
            0 => raw[0] = 1,
            1 => raw[4] = 0,
            2 => raw[8] = 2,
            3 => raw[36..40].copy_from_slice(&56u32.to_le_bytes()),
            4 => raw[36..40].copy_from_slice(&u32::MAX.to_le_bytes()),
            5 => *raw.last_mut().unwrap() = 0xff,
            _ => raw.truncate(60),
        }
        assert!(parse(&raw, 1).is_err(), "kind {kind}");
    }
    assert!(parse(&record(u32::MAX, "HKKbdFltr", &[]), u32::MAX).is_err());
}
