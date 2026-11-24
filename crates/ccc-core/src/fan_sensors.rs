use serde::Serialize;

use crate::fan::oem_tachometer_rpm;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanReading {
    pub rpm: Option<f64>,
    pub duty_pct: Option<f64>,
    pub temp_c: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct FanReadings {
    pub cpu: FanReading,
    pub gpu1: FanReading,
    pub gpu2: FanReading,
    pub sys: FanReading,
}

/// The two-point FanCount layout has three contiguous slots. A system fan requires a presence bit.
pub fn fan_channels(info: &[u8; 256]) -> Option<u8> {
    let count = info[12];
    if !(1..=4).contains(&count) {
        return None;
    }
    match info[50] & 0x0f {
        0 if count <= 3 => Some((1 << count) - 1),
        0 => None,
        channels => Some(channels),
    }
}

impl FanReadings {
    pub fn decode(package: &[u8; 256], channels: u8) -> Option<Self> {
        if package.iter().all(|&b| b == 0) || package.iter().all(|&b| b == 0xff) {
            return None;
        }
        // FAN.Read_FanSpeed uses separate counters, duty and remote fields for each channel.
        let read = |mask, counter: usize, duty: usize, remote: usize| {
            if channels & mask == 0 {
                return FanReading::default();
            }
            let counter = u16::from_be_bytes([package[counter], package[counter + 1]]);
            FanReading {
                rpm: Some(oem_tachometer_rpm(counter)),
                duty_pct: Some(f64::from(package[duty]) * 100.0 / 255.0),
                temp_c: temperature(package[remote]),
            }
        };
        let mut cpu = read(1, 2, 16, 18);
        // OEM reads CPU remote even on layouts without a separate CPU fan.
        // Its fixed zero TDP branch reports the raw temperature.
        cpu.temp_c = temperature(package[18]);
        Some(Self {
            cpu,
            gpu1: read(2, 4, 19, 21),
            gpu2: read(4, 6, 22, 24),
            sys: read(8, 36, 38, 40),
        })
    }
}

fn temperature(raw: u8) -> Option<f64> {
    (1..150).contains(&raw).then(|| f64::from(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_slots_keep_their_own_temperatures_and_counters() {
        let mut package = [0; 256];
        package[2..8].copy_from_slice(&[0x01, 0xda, 0x01, 0xe0, 0x02, 0x00]);
        package[16] = 128;
        package[18] = 60;
        package[19] = 100;
        package[21] = 50;
        package[22] = 90;
        package[24] = 70;
        package[36..38].copy_from_slice(&600u16.to_be_bytes());
        package[38] = 80;
        package[40] = 35;
        let sample = FanReadings::decode(&package, 15).unwrap();
        assert_eq!(sample.cpu.rpm, Some(4549.0));
        assert_eq!(sample.gpu1.rpm, Some(4492.0));
        assert_eq!(sample.gpu2.rpm, Some(oem_tachometer_rpm(512)));
        assert_eq!(sample.sys.rpm, Some(oem_tachometer_rpm(600)));
        assert_eq!(sample.gpu2.duty_pct, Some(90.0 * 100.0 / 255.0));
        assert_eq!(sample.sys.duty_pct, Some(80.0 * 100.0 / 255.0));
        assert_eq!(sample.cpu.temp_c, Some(60.0));
        assert_eq!(sample.gpu1.temp_c, Some(50.0));
        assert_eq!(sample.gpu2.temp_c, Some(70.0));
        assert_eq!(sample.sys.temp_c, Some(35.0));
    }

    #[test]
    fn missing_channel_and_invalid_temperature_are_distinct_from_zero_rpm() {
        let mut package = [0; 256];
        package[18] = 55;
        package[24] = 65;
        package[40] = 0xff;
        let sample = FanReadings::decode(&package, 0b1001).unwrap();
        assert_eq!(sample.gpu2, FanReading::default());
        assert_eq!(sample.sys.rpm, Some(0.0));
        assert_eq!(sample.sys.temp_c, None);
        let unknown = FanReadings::decode(&package, 0).unwrap();
        assert_eq!(unknown.cpu.temp_c, Some(55.0));
        assert_eq!(unknown.cpu.rpm, None);
        assert_eq!(unknown.sys, FanReading::default());
        assert!(FanReadings::decode(&[0; 256], 15).is_none());
        assert!(FanReadings::decode(&[0xff; 256], 15).is_none());
    }

    #[test]
    fn channel_presence_is_an_explicit_bitmap_or_a_valid_two_point_count() {
        let mut info = [0; 256];
        assert!(fan_channels(&info).is_none());
        info[12] = 3;
        assert_eq!(fan_channels(&info), Some(0b0111));
        info[12] = 4;
        assert!(fan_channels(&info).is_none());
        info[50] = 0b1101;
        assert_eq!(fan_channels(&info), Some(0b1101));
        assert!(fan_channels(&[0xff; 256]).is_none());
    }
}
