use super::*;

fn curve() -> [RpmPoint; LEVELS] {
    std::array::from_fn(|index| RpmPoint {
        temp_c: [35, 40, 45, 50, 60, 65, 70, 80, 90, 100][index],
        rpm_pct: [20, 30, 40, 45, 50, 60, 70, 80, 90, 100][index],
    })
}

fn table(maximum: u16) -> Table {
    let mut table = [0x5a; TABLE_SIZE];
    table[10..12].copy_from_slice(&maximum.to_be_bytes());
    table
}

#[test]
fn read_and_write_have_different_first_table_offsets() {
    let mut packet = [0; 256];
    packet[..112].fill(0x31);
    packet[128..240].fill(0x72);
    let pair = Pair::read(40, &packet).unwrap();
    let write = pair.write_packet();
    assert_eq!(write[0], 41);
    assert!(write[1..16].iter().all(|b| *b == 0));
    assert_eq!(&write[16..128], &[0x31; 112]);
    assert_eq!(&write[128..240], &[0x72; 112]);
    let pair = Pair::read(42, &packet).unwrap();
    assert_eq!(pair.write_packet()[0], 43);
    assert_eq!(pair.fan_index(0), 2);
    assert_eq!(pair.fan_index(1), 3);
    assert!(Pair::read(41, &packet).is_err());
}

#[test]
fn encodes_fixed_oem_bytes_and_preserves_thermal_and_ramp_fields() {
    let before = table(5000);
    let encoded = encode_curve(&before, 1, 2, &curve()).unwrap();
    assert_eq!(encoded[0], 0x92);
    assert_eq!(&encoded[1..22], &before[1..22]);
    assert_eq!(&encoded[82..], &before[82..]);
    assert_eq!(&encoded[22..32], &[0, 36, 41, 46, 51, 61, 66, 71, 81, 91]);
    assert_eq!(&encoded[32..42], &[35, 40, 45, 50, 60, 65, 70, 80, 90, 100]);
    assert_eq!(&encoded[42..46], &[0x03, 0xe8, 0x05, 0xdc]);
    assert_eq!(&encoded[62..66], &[0x03, 0xe8, 0x05, 0xdc]);
    assert_eq!(&encoded[80..82], &[0x13, 0x88]);
    assert_eq!(decode_curve(&encoded).unwrap(), curve());
}

#[test]
fn uses_each_fans_maximum_and_rounds_midpoints_like_dotnet() {
    let cpu = encode_curve(&table(5000), 0, 0, &curve()).unwrap();
    let gpu = encode_curve(&table(6000), 1, 0, &curve()).unwrap();
    assert_eq!(&cpu[62..64], &[0x03, 0xe8]);
    assert_eq!(&gpu[62..64], &[0x04, 0xb0]);
    let odd = encode_curve(&table(5005), 0, 4, &curve()).unwrap();
    assert_eq!(odd[0], 0x80);
    assert_eq!(&odd[70..72], &[0x09, 0xc6]); // 50% of 5005 = 2502.5 -> 2502.
}

#[test]
fn rejects_bad_targets_before_any_encoding() {
    let mut points = curve();
    points[0].rpm_pct = 0;
    assert!(validate_curve(&points).is_err());
    points = curve();
    points[1].temp_c = points[0].temp_c;
    assert!(validate_curve(&points).is_err());
    points = curve();
    points[4].rpm_pct = 20;
    assert!(validate_curve(&points).is_err());
    points = curve();
    points[9].rpm_pct = 95;
    assert!(validate_curve(&points).is_err());
    points = curve();
    points[9].temp_c = 95;
    assert!(validate_curve(&points).is_ok());
    assert!(encode_curve(&table(0), 0, 0, &curve()).is_err());
    assert!(encode_curve(&table(5000), 4, 0, &curve()).is_err());
    assert!(encode_curve(&table(5000), 0, 5, &curve()).is_err());
}

#[test]
fn reads_existing_zero_percent_without_fabricating_the_new_twenty_percent_floor() {
    let mut raw = encode_curve(&table(5000), 0, 0, &curve()).unwrap();
    raw[62..64].fill(0);
    assert_eq!(decode_curve(&raw).unwrap()[0].rpm_pct, 0);
    raw[62..64].copy_from_slice(&6000u16.to_be_bytes());
    assert!(decode_curve(&raw).is_err());
}

#[test]
fn trusted_default_matches_oem_points_and_response_bytes() {
    let raw = encode_default(&table(5000), 3, 3).unwrap();
    assert_eq!(raw[0], 0xb3);
    assert_eq!(decode_curve(&raw).unwrap()[0].rpm_pct, 0);
    assert_eq!(&raw[82..92], &[26; 10]);
    assert_eq!(&raw[92..102], &[10; 10]);
    assert_eq!(&raw[102..112], &[0x61; 10]);
    assert!(validate_curve(&decode_curve(&raw).unwrap()).is_err());
}
