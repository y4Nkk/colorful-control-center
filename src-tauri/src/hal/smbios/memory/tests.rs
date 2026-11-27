use super::*;

fn raw(records: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![0, 3, 4, 0];
    let length: usize = records.iter().map(Vec::len).sum();
    bytes.extend((length as u32).to_le_bytes());
    bytes.extend(records.iter().flatten());
    bytes
}

fn array(handle: u16, usage: u8, count: u16) -> Vec<u8> {
    let mut data = vec![0; 17];
    data[..2].copy_from_slice(&[16, 15]);
    data[2..4].copy_from_slice(&handle.to_le_bytes());
    data[5] = usage;
    data[13..15].copy_from_slice(&count.to_le_bytes());
    data
}

fn device(handle: u16, array: u16, form: u8, size: u16, speed: u16) -> Vec<u8> {
    let mut data = vec![0; 0x5c];
    data[..2].copy_from_slice(&[17, 0x5c]);
    data[2..4].copy_from_slice(&handle.to_le_bytes());
    data[4..6].copy_from_slice(&array.to_le_bytes());
    data[0x0c..0x0e].copy_from_slice(&size.to_le_bytes());
    data[0x0e] = form;
    data[0x10] = 1;
    data[0x12] = 0x22;
    data[0x17] = 2;
    data[0x18] = 3;
    data[0x19] = 4;
    data[0x1a] = 5;
    data[0x20..0x22].copy_from_slice(&speed.to_le_bytes());
    data.extend(b"DIMM 0\0Vendor\0secret-serial\0secret-asset\0Part A\0\0");
    data
}

#[test]
fn system_arrays_select_modules_and_count_only_physical_sockets() {
    let data = raw(&[
        device(7, 1, 0x0d, 16384, 6400),
        array(1, 3, 2),
        array(2, 3, 1),
        array(3, 4, 1),
        device(8, 1, 0x0d, 0, 0),
        device(9, 2, 0x0b, 8192, 6400),
        device(10, 3, 0x09, 4096, 3200),
    ]);
    let inv = inventory(&data).unwrap();
    assert_eq!(inv.speed_mts, Some(6400));
    assert_eq!(inv.slots, Some(MemorySlots { used: 1, total: 2 }));
    assert_eq!(inv.modules.len(), 2);
    assert_eq!(inv.modules[0].capacity_bytes, Some(16 << 30));
    assert_eq!(inv.modules[1].soldered, Some(true));
    let json = serde_json::to_string(&inv.modules).unwrap();
    assert!(json.contains("DIMM 0") && json.contains("Part A") && !json.contains("secret"));
}

#[test]
fn mixed_or_missing_speeds_keep_modules_and_do_not_substitute_rated_speed() {
    for speed in [0, 4800, 0xffff] {
        let data = raw(&[
            array(1, 3, 2),
            device(7, 1, 0x09, 16384, 6400),
            device(8, 1, 0x09, 0xffff, speed),
        ]);
        let inv = inventory(&data).unwrap();
        assert_eq!(inv.speed_mts, None);
        assert_eq!(inv.modules.len(), 2);
        assert_eq!(inv.modules[1].capacity_bytes, None);
        assert_eq!(inv.slots, Some(MemorySlots { used: 2, total: 2 }));
    }
}

#[test]
fn size_and_speed_extensions_apply_only_to_their_sentinels() {
    let mut large = device(7, 1, 0x09, 0x7fff, 0xffff);
    large[0x1c..0x20].copy_from_slice(&65536u32.to_le_bytes());
    large[0x58..0x5c].copy_from_slice(&70000u32.to_le_bytes());
    let inv = inventory(&raw(&[array(1, 3, 1), large.clone()])).unwrap();
    assert_eq!(inv.modules[0].capacity_bytes, Some(64 << 30));
    assert_eq!(inv.speed_mts, Some(70000));
    large[0x0c..0x0e].copy_from_slice(&0x8100u16.to_le_bytes());
    large[0x20..0x22].copy_from_slice(&5200u16.to_le_bytes());
    let inv = inventory(&raw(&[array(1, 3, 1), large])).unwrap();
    assert_eq!(inv.modules[0].capacity_bytes, Some(256 << 10));
    assert_eq!(inv.speed_mts, Some(5200));
}

#[test]
fn incomplete_arrays_and_unknown_form_factors_do_not_claim_socket_counts() {
    let data = raw(&[array(1, 3, 2), device(7, 1, 0x09, 16384, 6400)]);
    let inv = inventory(&data).unwrap();
    assert_eq!(inv.modules.len(), 1);
    assert_eq!(inv.slots, None);
    assert_eq!(inv.speed_mts, None);
    let data = raw(&[array(1, 3, 1), device(7, 1, 0x02, 16384, 6400)]);
    let inv = inventory(&data).unwrap();
    assert_eq!(inv.modules.len(), 1);
    assert_eq!(inv.slots, None);
    assert_eq!(inv.speed_mts, Some(6400));
    assert!(inventory(&data[..data.len() - 1]).is_err());
}
