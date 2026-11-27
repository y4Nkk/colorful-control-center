use super::*;

pub(super) const FACTORY_CURVES: [[u8; 14]; 3] = [
    [40, 60, 80, 100, 51, 102, 204, 255, 0, 40, 0, 82, 0, 41],
    [40, 60, 80, 100, 51, 102, 204, 255, 0, 39, 0, 83, 0, 42],
    [0; 14],
];

unsafe extern "system" fn buffer(command: i32, out: *mut u8) -> i32 {
    let mut packet = [0; 257];
    match command {
        12 => {
            packet[18] = device().0.lock().unwrap().cpu_temperature;
            packet[16] = 76;
            packet[19] = 76;
        }
        13 => {
            let d = device();
            let mut state = d.0.lock().unwrap();
            packet[15] = state.pages[2][4];
            packet[43] = u8::from(state.keyboard_boot_supported);
            packet[12] = state.fan_count;
            packet[14] = ccc_core::fan::oem_fan_mode_bits(state.fan_mode).unwrap_or(0);
            if let Some(bits) = state.fan_readbacks.pop_front() {
                packet[14] = bits;
            }
            packet[50] = (1 << state.fan_count) - 1;
            for (index, curve) in state.fan_curves.iter().enumerate() {
                for point in 0..4 {
                    packet[16 + index * 8 + point * 2] = curve[point];
                    packet[17 + index * 8 + point * 2] = curve[4 + point];
                }
            }
        }
        17 | 18 => (),
        _ => return 0,
    }
    unsafe {
        std::ptr::copy_nonoverlapping(packet.as_ptr(), out, packet.len());
    }
    command
}
unsafe extern "system" fn read(page: i32, offset: i32, length: i32, out: *mut u8) -> i32 {
    let d = device();
    let state = d.0.lock().unwrap();
    unsafe {
        std::ptr::copy_nonoverlapping(
            state.pages[page as usize][offset as usize..].as_ptr(),
            out,
            length as usize,
        );
    }
    4096
}
unsafe extern "system" fn write(page: i32, offset: i32, length: i32, bytes: *const u8) -> i32 {
    let d = device();
    let mut state = d.0.lock().unwrap();
    let bytes = unsafe { std::slice::from_raw_parts(bytes, length as usize) };
    state.events.push(format!("PAGE{page}:{offset}:{bytes:?}"));
    state.pages[page as usize][offset as usize..offset as usize + bytes.len()]
        .copy_from_slice(bytes);
    if page == 1 && offset == 1 && state.fail_mode_save {
        state.fail_mode_save = false;
        return 0;
    }
    if state.fail_option_save == Some((page as u8, offset as usize)) {
        state.fail_option_save = None;
        return 0;
    }
    if page == 6 && offset == 69 && state.fail_saved {
        state.fail_saved = false;
        return 0;
    }
    if page == 4 && offset == 81 && state.fail_dust_save {
        state.fail_dust_save = false;
        return 0;
    }
    if page == 1 && offset == 36 && state.fail_keyboard_save {
        state.fail_keyboard_save = false;
        return 0;
    }
    if page == 2 && state.fail_page2_at == Some(offset as usize) {
        state.fail_page2_at = None;
        return 0;
    }
    1
}
unsafe extern "system" fn set(command: i32, bytes: *const u8, length: i32) -> i32 {
    let d = device();
    let mut state = d.0.lock().unwrap();
    let bytes = unsafe { std::slice::from_raw_parts(bytes, length as usize) };
    if command == 79 {
        if !energy_packet::set(&mut state, bytes[0]) {
            return 0;
        }
    } else if matches!(command, 39 | 103) {
        let word = u32::from_le_bytes(bytes[..4].try_into().unwrap());
        state.events.push(format!("LIGHT:{command}:{word:08x}"));
        if state.fail_lighting_words != 0 {
            state.fail_lighting_words -= 1;
            return 0;
        }
    } else if command == 121 && matches!(bytes[3], 40 | 41) {
        let sub = bytes[3];
        let value = u32::from_le_bytes(bytes[..4].try_into().unwrap()) & 0xffffff;
        state.events.push(format!("DUST:{sub}:{value}"));
        if state.fail_dust_commands != 0 {
            state.fail_dust_commands -= 1;
            return 0;
        }
    } else if command == 121 && bytes[3] == 25 {
        let mode = bytes[0];
        state.events.push(format!("MODE:{mode}"));
        state.fan_readbacks = std::mem::take(&mut state.fan_readbacks_on_preset);
        state.power = ((1040 | 0x8000) << 32) | (720 | 0x8000);
        state.voltage = [-20, -21, 0];
        if state.fail_mode {
            return 0;
        }
    } else if command == 14 {
        return set_curve(&mut state, bytes);
    } else if command == 121 && matches!(bytes[3], 1 | 14 | 34) {
        return set_fan(&mut state, bytes[3], bytes[0]);
    } else {
        if command == 4 && bytes.get(6) == Some(&0xc1) {
            state.direct_control[usize::from(bytes[1] - 1)] = DirectControl::Manual(bytes[2]);
        } else if command == 0x69 {
            for (index, control) in state.direct_control.iter_mut().enumerate() {
                if bytes[0] & (1 << index) != 0 {
                    *control = DirectControl::Firmware;
                }
            }
        }
        state.events.push(format!("DCHU:{command}"));
    }
    command
}

fn set_fan(state: &mut State, sub: u8, value: u8) -> i32 {
    match sub {
        1 => {
            state.events.push(format!("FAN_MODE:{value}"));
            if std::mem::take(&mut state.fail_fan_mode) {
                state.events.push("FAN_MODE_FAILURE".into());
                return 0;
            }
            state.fan_command = Some(value);
            state.fan_readbacks = state
                .fan_readbacks_on_commands
                .pop_front()
                .unwrap_or_default();
            if state.ignored_fan_writes == 0 {
                state.fan_mode = value;
            } else {
                state.ignored_fan_writes -= 1;
            }
        }
        14 => state.offset_raw = value,
        34 => {
            state.events.push("FAN_RESET".into());
            if std::mem::take(&mut state.fail_default) {
                state.fan_mode = 0;
                return 0;
            }
            state.fan_curves = FACTORY_CURVES;
        }
        _ => unreachable!(),
    }
    state.events.push("DCHU:121".into());
    121
}

fn set_curve(state: &mut State, bytes: &[u8]) -> i32 {
    state.events.push("DCHU:14".into());
    if state.fail_curve_commands > 0 {
        state.fail_curve_commands -= 1;
        return 0;
    }
    for (index, curve) in state.fan_curves.iter_mut().enumerate() {
        let at = 2 + index * 4;
        [curve[1], curve[5], curve[2], curve[6]] = bytes[at..at + 4].try_into().unwrap();
        curve[8..].copy_from_slice(&bytes[14 + index * 6..20 + index * 6]);
    }
    0x14
}
pub fn io() -> Dchu {
    Dchu::test_io(buffer, read, set, write).with_test_exchange(exchange)
}

unsafe extern "system" fn exchange(command: i32, input: *const u8, _: i32, out: *mut u8) -> i32 {
    let selector = unsafe { *input };
    let d = device();
    let mut state = d.0.lock().unwrap();
    if matches!(selector, 24 | 25) {
        if selector == 25 {
            let values: [u8; 5] = unsafe {
                std::slice::from_raw_parts(input.add(1), 5)
                    .try_into()
                    .unwrap()
            };
            state.keyboard_live = values;
            state.events.push(format!("KEYBOARD:{values:?}"));
            if state.fail_keyboard_send != 0 {
                state.fail_keyboard_send -= 1;
                return 0;
            }
        }
        unsafe {
            std::ptr::write_bytes(out, 0, 257);
            std::ptr::copy_nonoverlapping(state.keyboard_live.as_ptr(), out, 5);
            *out.add(256) = state.keyboard_live[0];
        }
        return command;
    }
    if selector == 1 && unsafe { *input.add(1) } == 163 {
        unsafe {
            std::ptr::write_bytes(out, 0, 257);
            *out = 0xb8;
            *out.add(1) = u8::from(state.dust_running);
            *out.add(256) = 0xb8;
        }
        return command;
    }
    if matches!(selector, 3 | 5) {
        let packet = fan_window(&state, selector);
        unsafe { std::ptr::copy_nonoverlapping(packet.as_ptr(), out, packet.len()) };
        return command;
    }
    if matches!(selector, 41 | 43) {
        let input = unsafe { std::slice::from_raw_parts(input, 256) };
        return write_rpm(&mut state, selector, input, out);
    }
    if selector == 54 || selector == 55 {
        if selector == 55 {
            state.pd_level[0] = unsafe { *input.add(1) };
            let level = state.pd_level[0];
            state.events.push(format!("PD:{level}"));
            if state.fail_pd != 0 {
                state.fail_pd -= 1;
                return 0;
            }
        }
        unsafe {
            std::ptr::write_bytes(out, 0, 257);
            std::ptr::copy_nonoverlapping(state.pd_level.as_ptr(), out, 2);
            *out.add(256) = state.pd_level[0];
        }
        return command;
    }
    let table = if selector == 8 {
        Some(&state.bios)
    } else {
        state.policy_tables.get(&selector)
    };
    let Some(table) = table else {
        return 0;
    };
    unsafe {
        std::ptr::copy_nonoverlapping(table.as_ptr(), out, 256);
        *out.add(256) = table[0];
    }
    command
}

fn fan_window(state: &State, selector: u8) -> [u8; 257] {
    let mut output = [0; 257];
    if selector == 3 {
        output[0xd9] = 0x80;
        output[0xe7] = state.offset_raw;
    } else {
        output[10] = state.fan_count;
        for (index, curve) in state.fan_curves.iter().enumerate() {
            let at = 12 + index * 14;
            output[at..at + 14].copy_from_slice(curve);
        }
    }
    output
}

fn write_rpm(state: &mut State, selector: u8, input: &[u8], output: *mut u8) -> i32 {
    let Some(table) = state.policy_tables.get_mut(&(selector - 1)) else {
        return 0;
    };
    table[..112].copy_from_slice(&input[16..128]);
    table[128..240].copy_from_slice(&input[128..240]);
    if std::mem::take(&mut state.fail_rpm_write) {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(table.as_ptr(), output, 256);
        *output.add(256) = table[0];
    }
    4
}
