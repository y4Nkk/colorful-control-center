use serde::Serialize;

pub const EVENTS: [u8; 11] = [78, 79, 48, 49, 50, 51, 52, 53, 54, 55, 56];
pub fn target_index(event: u8) -> Option<usize> {
    EVENTS.iter().position(|candidate| *candidate == event)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Targets(pub [u8; 11]);
impl Targets {
    pub fn from_packet(packet: &[u8; 256]) -> Result<Self, String> {
        let values: [u8; 11] = packet[210..221].try_into().unwrap();
        if values.iter().any(|value| *value > 100) {
            return Err("原厂屏幕亮度目标超出百分比范围".into());
        }
        Ok(Self(values))
    }
    pub fn for_event(&self, event: u8) -> Result<u8, String> {
        target_index(event)
            .map(|index| self.0[index])
            .ok_or_else(|| "未知屏幕亮度策略事件".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_event_table_uses_all_eleven_original_offsets() {
        let mut packet = [255; 256];
        packet[210..221].copy_from_slice(&[100, 0, 90, 80, 70, 60, 50, 40, 30, 20, 10]);
        let targets = Targets::from_packet(&packet).unwrap();
        for (index, event) in EVENTS.into_iter().enumerate() {
            assert_eq!(target_index(event), Some(index));
            assert_eq!(targets.for_event(event).unwrap(), packet[210 + index]);
        }
        for event in [0, 47, 57, 77, 80, 138, 215] {
            assert!(targets.for_event(event).is_err());
        }
    }
    #[test]
    fn invalid_target_is_rejected_without_clamping_and_zero_is_a_real_target() {
        let mut packet = [0; 256];
        assert_eq!(Targets::from_packet(&packet).unwrap().0, [0; 11]);
        for index in 210..221 {
            packet[index] = 101;
            assert!(Targets::from_packet(&packet).is_err());
            packet[index] = 0;
        }
    }
}
