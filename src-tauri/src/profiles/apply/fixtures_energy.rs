use super::State;

pub(super) fn hardware() -> [u8; 256] {
    let mut packet = [0; 256];
    packet[0] = 1;
    packet[9] = 0x40;
    packet
}
pub(super) fn software(value: u8) -> [u8; 256] {
    let mut packet = [0; 256];
    packet[0] = 1;
    packet[2] = value << 4;
    packet[10] = 1;
    packet
}
pub(super) fn set(state: &mut State, value: u8) -> bool {
    state.events.push(format!("ENERGY:{value}"));
    if state.fail_energy == Some(value) {
        state.fail_energy = None;
        return false;
    }
    if !state.ignore_energy_write {
        let packet = &mut state.bios;
        packet[2] = packet[2] & !0x30 | value << 4;
    }
    true
}
