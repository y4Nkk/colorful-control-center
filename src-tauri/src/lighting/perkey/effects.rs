use super::*;
pub(super) fn dynamic(s: &Snapshot) -> Result<Vec<[u8; 5]>> {
    let at = |n: usize| s.at(n);
    let color = |n: usize| [at(n), at(n + 1), at(n + 2)];
    let packet = |cmd: u8, param: u8, c: [u8; 3]| [cmd, param, c[0], c[1], c[2]];
    let choose = |flag: usize, cmd: u8, color_at: usize| {
        if at(flag) == 0 {
            [cmd, 0, 0, 0, 0]
        } else {
            packet(cmd, 170, color(color_at))
        }
    };
    Ok(match at(32) {
        0 => vec![[0, 4, 0, 0, 0]],
        1 => vec![choose(52, 10, 53)],
        2 => vec![choose(56, 11, 57)],
        3 | 9 => {
            if at(32) == 3 || at(76) == 0 {
                vec![[0, 9, 0, 0, 0]]
            } else {
                vec![packet(24, 161, color(77))]
            }
        }
        4 => vec![[0, 10, 0, 0, 0]],
        5 => vec![[7, 0, 0, 0, 0]],
        6 => vec![[0, 11, 0, 0, 0]],
        7 => {
            let direction = at(64);
            if direction > 15 {
                anyhow::bail!("未知逐键波浪方向");
            }
            vec![packet(
                21,
                if direction < 8 {
                    113 + direction
                } else {
                    161 + direction - 8
                },
                if direction < 8 { [0; 3] } else { color(66) },
            )]
        }
        8 => {
            if at(69) == 0 {
                vec![[0, 10, 0, 0, 0]]
            } else {
                vec![packet(23, 161, color(70)), packet(23, 162, color(73))]
            }
        }
        10 => {
            let direction = at(60);
            if direction > 7 {
                anyhow::bail!("未知逐键蛇形方向");
            }
            vec![packet(
                22,
                if direction < 4 {
                    113 + direction
                } else {
                    161 + direction - 4
                },
                if direction < 4 { [0; 3] } else { color(61) },
            )]
        }
        11 | 12 => vec![[0, 12, 0, 0, 0]],
        13 => vec![[7, 0, 0, 0, 0]],
        _ => anyhow::bail!("未知逐键动态模式，不能保证恢复"),
    })
}

pub(super) fn values(s: &Snapshot) -> Result<Values> {
    let optional = |at: usize| {
        if s.at(at) == 0 {
            None
        } else {
            Some(s.rgb(at + 1))
        }
    };
    let pattern = if s.at(33) == 1 || s.at(32) == 12 {
        Pattern::Static { colors: s.colors() }
    } else {
        match s.at(32) {
            0 => Pattern::Wave {
                direction: 0,
                color: None,
            },
            1 => Pattern::Breath {
                color: optional(52),
            },
            2 => Pattern::Blink {
                color: optional(56),
            },
            3 => Pattern::Random { color: None },
            4 => Pattern::Scan { colors: None },
            5 => Pattern::Ripple {
                reactive: s.at(48) >= 2,
                color: (s.at(48) & 1 != 0).then(|| s.rgb(49)),
            },
            6 => Pattern::Snake {
                direction: 0,
                color: None,
            },
            7 => Pattern::Wave {
                direction: s.at(64) % 8,
                color: (s.at(64) >= 8).then(|| s.rgb(66)),
            },
            8 => Pattern::Scan {
                colors: (s.at(69) != 0).then(|| [s.rgb(70), s.rgb(73)]),
            },
            9 => Pattern::Random {
                color: optional(76),
            },
            10 => Pattern::Snake {
                direction: s.at(60) % 4,
                color: (s.at(60) >= 4).then(|| s.rgb(61)),
            },
            11 => Pattern::Fn { color: s.rgb(45) },
            13 => Pattern::Fade {
                color: optional(41),
            },
            _ => anyhow::bail!("逐键模式未知"),
        }
    };
    Ok(Values {
        brightness: s.at(35),
        enabled: s.at(80) == 1,
        speed: s.at(34),
        pattern,
    })
}
pub(super) fn presets(s: &Snapshot, device: &device::DeviceInfo) -> Result<Vec<Pattern>> {
    let mut result = vec![Pattern::Static { colors: s.colors() }];
    for mode in if device.new_effects {
        vec![7, 1, 2, 9, 8, 10, 5, 11, 13]
    } else {
        vec![0, 1, 2, 3, 4, 6, 5, 11, 13]
    } {
        let mut p = s.clone();
        p.put(32, mode);
        p.put(33, 0);
        result.push(values(&p)?.pattern);
    }
    Ok(result)
}
