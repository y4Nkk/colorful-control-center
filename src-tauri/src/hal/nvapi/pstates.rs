//! PSTATES20 读写视图与写入包构造（纯函数，测试不触碰 GPU/DLL）。
//!
//! 读取协议（与 LenovoLegionToolkit / g-helper 的生产实现一致）：
//! 当前偏移 = P0 pstate 中 domainId∈{GRAPHICS=0, MEMORY=4} 条目的
//! freqDelta.value（kHz）；驱动允许区间 = freqDelta.valueRange。
//!
//! 写入协议（最小 V1 包，见 docs/nvapi-contract.md §3）：
//! 只携带 numPstates=1（P0）+ 请求的时钟域增量条目，
//! 不包含任何绝对频率 / 电压 / 其它 pstate / 标志——
//! 未列出的字段由驱动保持原状，杜绝整表回写覆盖。

use super::ffi::*;

/// 单个时钟域的读数（kHz）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DomainReading {
    /// 是否在 P0 的时钟表中找到该域
    pub present: bool,
    /// 条目 bIsEditable（bit0）
    pub editable: bool,
    /// 当前增量（kHz）
    pub delta_khz: i32,
    /// 驱动允许的最小增量（kHz）
    pub min_khz: i32,
    /// 驱动允许的最大增量（kHz）
    pub max_khz: i32,
}

/// GetPstates20 结果的域视图。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParsedPstates {
    /// 顶层 bIsEditable（驱动整体 OC 能力）
    pub editable: bool,
    /// 表内是否存在 P0
    pub p0_present: bool,
    pub core: DomainReading,
    pub memory: DomainReading,
    /// 驱动返回的计数字段越界（已钳制，界面告警用）
    pub count_clamped: bool,
}

/// 计数字段安全截取：驱动返回值超出数组容量时按容量处理并标记。
fn clamp_count(value: u32, max: usize) -> (usize, bool) {
    if value as usize > max {
        (max, true)
    } else {
        (value as usize, false)
    }
}

fn read_domain(p0: &PStateEntry, num_clocks: usize, domain: u32) -> DomainReading {
    for clock in &p0.clocks[..num_clocks] {
        if clock.domain_id == domain {
            return DomainReading {
                present: true,
                editable: clock.flags & 1 == 1,
                delta_khz: clock.freq_delta_khz.value,
                min_khz: clock.freq_delta_khz.min,
                max_khz: clock.freq_delta_khz.max,
            };
        }
    }
    DomainReading::default()
}

/// 解析 GetPstates20 结果（只读，不修改入参）。
pub fn parse(info: &Pstates20Info) -> ParsedPstates {
    let (num_pstates, clamped_p) = clamp_count(info.num_pstates, MAX_PSTATES);
    let (num_clocks, clamped_c) = clamp_count(info.num_clocks, MAX_CLOCKS);
    let mut out = ParsedPstates {
        editable: info.flags & 1 == 1,
        count_clamped: clamped_p || clamped_c,
        ..Default::default()
    };
    for pstate in &info.pstates[..num_pstates] {
        if pstate.pstate_id == PSTATE_P0 {
            out.p0_present = true;
            out.core = read_domain(pstate, num_clocks, CLOCK_GRAPHICS);
            out.memory = read_domain(pstate, num_clocks, CLOCK_MEMORY);
            out.core.editable &= out.editable && pstate.flags & 1 == 1;
            out.memory.editable &= out.editable && pstate.flags & 1 == 1;
            break;
        }
    }
    out
}

/// 构造最小写入包（V1）：只含请求的增量条目。
///
/// `None` 的域不进入时钟表——驱动不会触碰未列出的域。
/// 除下列字段外全部为零且**不会被驱动解释**（numClocks 划定有效范围）：
/// version、numPstates=1、pstates[0].pstateId=P0、numClocks=n、
/// clocks[i].domainId/typeId=0(Single)/freqDelta.value。
pub fn build_set_request(core_khz: Option<i32>, memory_khz: Option<i32>) -> Pstates20Info {
    let mut req = Pstates20Info::zeroed();
    req.version = VERSION_V1;
    req.num_pstates = 1;
    req.num_base_voltages = 0;
    let p0 = &mut req.pstates[0];
    p0.pstate_id = PSTATE_P0;
    let mut count = 0usize;
    for (domain, khz) in [(CLOCK_GRAPHICS, core_khz), (CLOCK_MEMORY, memory_khz)] {
        if let Some(delta) = khz {
            p0.clocks[count] = ClockEntry {
                domain_id: domain,
                type_id: CLOCK_TYPE_SINGLE,
                flags: 0,
                freq_delta_khz: ParamDelta {
                    value: delta,
                    min: 0,
                    max: 0,
                },
                data: [0; 5],
            };
            count += 1;
        }
    }
    req.num_clocks = count as u32;
    req
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个形如真机 GetPstates20 的表：P0 可编辑 + GRAPHICS/MEMORY 域。
    fn sample_info() -> Pstates20Info {
        let mut info = Pstates20Info::zeroed();
        info.version = VERSION_V3;
        info.flags = 1; // 顶层可编辑
        info.num_pstates = 2;
        info.num_clocks = 2;
        info.num_base_voltages = 1;
        let p0 = &mut info.pstates[0];
        p0.pstate_id = PSTATE_P0;
        p0.flags = 1;
        p0.clocks[0] = ClockEntry {
            domain_id: CLOCK_GRAPHICS,
            type_id: 0,
            flags: 1,
            freq_delta_khz: ParamDelta {
                value: 135_000,
                min: -200_000,
                max: 400_000,
            },
            data: [2_100_000, 0, 0, 0, 0],
        };
        p0.clocks[1] = ClockEntry {
            domain_id: CLOCK_MEMORY,
            type_id: 0,
            flags: 1,
            freq_delta_khz: ParamDelta {
                value: 0,
                min: -1_000_000,
                max: 2_000_000,
            },
            data: [6_001_000, 0, 0, 0, 0],
        };
        p0.base_voltages[0] = BaseVoltageEntry {
            domain_id: 0,
            flags: 1,
            volt_uv: 1_050_000,
            volt_delta_uv: ParamDelta {
                value: 0,
                min: 0,
                max: 0,
            },
        };
        let p8 = &mut info.pstates[1];
        p8.pstate_id = 8;
        p8.flags = 0;
        p8.clocks[0] = ClockEntry {
            domain_id: CLOCK_GRAPHICS,
            type_id: 0,
            flags: 0,
            freq_delta_khz: ParamDelta {
                value: 0,
                min: 0,
                max: 0,
            },
            data: [210_000, 0, 0, 0, 0],
        };
        info.num_ov_voltages = 0;
        info
    }

    #[test]
    fn parse_reads_p0_offsets_and_ranges() {
        let parsed = parse(&sample_info());
        assert!(parsed.editable && parsed.p0_present && !parsed.count_clamped);
        assert_eq!(
            parsed.core,
            DomainReading {
                present: true,
                editable: true,
                delta_khz: 135_000,
                min_khz: -200_000,
                max_khz: 400_000,
            }
        );
        assert_eq!(parsed.memory.delta_khz, 0);
        assert_eq!(parsed.memory.min_khz, -1_000_000);
        assert_eq!(parsed.memory.max_khz, 2_000_000);
    }

    #[test]
    fn parse_handles_missing_p0_and_domains() {
        let mut info = Pstates20Info::zeroed();
        info.num_pstates = 1;
        info.num_clocks = 1;
        info.pstates[0].pstate_id = 8; // 只有 P8
        let parsed = parse(&info);
        assert!(!parsed.p0_present);
        assert_eq!(parsed.core, DomainReading::default());
        assert_eq!(parsed.memory, DomainReading::default());

        // 有 P0 但时钟表里没有 MEMORY 域
        let mut info2 = Pstates20Info::zeroed();
        info2.num_pstates = 1;
        info2.num_clocks = 1;
        info2.pstates[0].pstate_id = PSTATE_P0;
        info2.pstates[0].clocks[0].domain_id = CLOCK_GRAPHICS;
        let parsed2 = parse(&info2);
        assert!(parsed2.p0_present);
        assert!(parsed2.core.present);
        assert!(!parsed2.memory.present);
    }

    #[test]
    fn parse_clamps_bogus_counts() {
        let mut info = sample_info();
        info.num_pstates = 99;
        info.num_clocks = 42;
        let parsed = parse(&info);
        assert!(parsed.count_clamped);
        // P0 仍在有效范围内，读数不 panic
        assert_eq!(parsed.core.delta_khz, 135_000);
    }

    #[test]
    fn parse_respects_editable_bits() {
        let mut info = sample_info();
        info.flags = 0; // 顶层不可编辑
        info.pstates[0].clocks[1].flags = 0; // MEMORY 条目不可编辑
        let parsed = parse(&info);
        assert!(!parsed.editable);
        assert!(!parsed.core.editable);
        assert!(!parsed.memory.editable);
    }

    /// 写入包契约：V1、单 pstate=P0、只含请求域、增量值精确、其余字段不携带。
    #[test]
    fn build_set_request_minimal_v1_shape() {
        let req = build_set_request(Some(150_000), Some(1_000_000));
        assert_eq!(req.version, VERSION_V1);
        assert_eq!(req.num_pstates, 1);
        assert_eq!(req.num_clocks, 2);
        assert_eq!(req.num_base_voltages, 0);
        assert_eq!(req.num_ov_voltages, 0);
        assert_eq!(req.pstates[0].pstate_id, PSTATE_P0);
        assert_eq!(req.pstates[0].flags, 0);
        let core = &req.pstates[0].clocks[0];
        assert_eq!(core.domain_id, CLOCK_GRAPHICS);
        assert_eq!(core.type_id, CLOCK_TYPE_SINGLE);
        assert_eq!(core.freq_delta_khz.value, 150_000);
        assert_eq!(core.freq_delta_khz.min, 0);
        assert_eq!(core.freq_delta_khz.max, 0);
        let mem = &req.pstates[0].clocks[1];
        assert_eq!(mem.domain_id, CLOCK_MEMORY);
        assert_eq!(mem.freq_delta_khz.value, 1_000_000);
        // 时钟表外的条目不被驱动解释（numClocks=2 划定有效范围）
        assert_eq!(req.pstates[0].clocks[2].domain_id, 0);
        assert_eq!(req.pstates[0].base_voltages[0].volt_uv, 0);
    }

    #[test]
    fn build_set_request_omits_none_domains() {
        let req = build_set_request(Some(-30_000), None);
        assert_eq!(req.num_clocks, 1);
        assert_eq!(req.pstates[0].clocks[0].domain_id, CLOCK_GRAPHICS);
        assert_eq!(req.pstates[0].clocks[0].freq_delta_khz.value, -30_000);

        let both_none = build_set_request(None, None);
        assert_eq!(both_none.num_clocks, 0);
        assert_eq!(both_none.num_pstates, 1);
    }
}
