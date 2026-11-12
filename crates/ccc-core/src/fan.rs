//! 风扇曲线引擎（纯逻辑，无硬件访问，单元测试覆盖）。
//!
//! 两种执行模式（docs/oem-dchu-contract.md §5）：
//! - **OEM 曲线模式**（Phase 2 交付）：用户曲线压缩为 OEM 3 可调点
//!   （T2/D2、T3/D3，T1/D1 与 100/100 固定）经 cmd14 写入固件——固件执行
//!   曲线，软件崩溃不影响风扇，天然安全
//! - **软件闭环模式**（后续）：引擎逐 tick 输出 duty 直写 EC（0xE7），
//!   需要内核通道 + 看门狗；本引擎的插值/滞后/斜坡逻辑为该模式而写

/// 曲线控制点。duty 以 % 计（写入钳位 ≥20% 防失速，安全设计 docs/design.md §4）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvePoint {
    pub temp_c: f64,
    pub duty_pct: f64,
}

#[derive(Clone, Debug)]
pub struct FanCurve {
    /// 按 temp 升序；首点 temp 为起始温度，末点之后保持末点 duty
    pub points: Vec<CurvePoint>,
    /// 滞后回差：温度下降超过该值才允许 duty 回落（°C）
    pub hysteresis_c: f64,
    /// 斜坡限制：每秒最大 duty 变化（百分点/秒）
    pub ramp_pct_per_s: f64,
}

pub const MIN_DUTY_PCT: f64 = 20.0;
pub const MAX_DUTY_PCT: f64 = 100.0;
/// 温度熔断阈值：超过即强制全速（安全设计）
pub const THERMAL_FUSE_C: f64 = 95.0;

impl FanCurve {
    /// 构造并规范化：按温度排序、去重、钳位 duty ≥ MIN_DUTY。
    pub fn new(mut points: Vec<CurvePoint>, hysteresis_c: f64, ramp_pct_per_s: f64) -> Self {
        points.sort_by(|a, b| a.temp_c.total_cmp(&b.temp_c));
        points.dedup_by(|a, b| (a.temp_c - b.temp_c).abs() < f64::EPSILON);
        for p in &mut points {
            p.duty_pct = p.duty_pct.clamp(MIN_DUTY_PCT, MAX_DUTY_PCT);
        }
        Self { points, hysteresis_c, ramp_pct_per_s }
    }

    /// 分段线性插值；低于首点取首点 duty，高于末点取末点 duty。
    pub fn interpolate(&self, temp_c: f64) -> f64 {
        let points = &self.points;
        if points.is_empty() {
            return MIN_DUTY_PCT;
        }
        if temp_c <= points[0].temp_c {
            return points[0].duty_pct;
        }
        let last = points[points.len() - 1];
        if temp_c >= last.temp_c {
            return last.duty_pct;
        }
        for window in points.windows(2) {
            let (a, b) = (window[0], window[1]);
            if temp_c <= b.temp_c {
                let t = (temp_c - a.temp_c) / (b.temp_c - a.temp_c);
                return a.duty_pct + t * (b.duty_pct - a.duty_pct);
            }
        }
        last.duty_pct
    }

    /// 单 tick 评估：输入 max(CPU, GPU) 温度与上次输出，输出本 tick duty。
    /// 规则：插值 → 熔断（>95°C 全速）→ 滞后回差 → 斜坡限制 → 钳位。
    pub fn evaluate(&self, temp_c: f64, last_duty: f64, dt_s: f64) -> f64 {
        let mut target = self.interpolate(temp_c);
        if temp_c >= THERMAL_FUSE_C {
            target = MAX_DUTY_PCT;
        }
        // 滞后回差：目标比上次输出低时，要求温度已回落超过 hysteresis 才跟随
        let last_point_temp = self.temp_for_duty(last_duty);
        if target < last_duty && (last_point_temp - temp_c) < self.hysteresis_c {
            target = last_duty;
        }
        // 斜坡限制
        let max_step = self.ramp_pct_per_s * dt_s.max(0.0);
        let next = last_duty + (target - last_duty).clamp(-max_step, max_step);
        next.clamp(MIN_DUTY_PCT, MAX_DUTY_PCT)
    }

    /// 反查：达到某 duty 的最低温度点（滞后判断用；用插值上限逼近）。
    fn temp_for_duty(&self, duty: f64) -> f64 {
        let mut result = self
            .points
            .first()
            .map(|p| p.temp_c)
            .unwrap_or(THERMAL_FUSE_C);
        for p in &self.points {
            if p.duty_pct <= duty {
                result = result.max(p.temp_c);
            }
        }
        result
    }
}

/// OEM cmd14 曲线点转换：把任意曲线压缩为 OEM 3 可调点
/// （T2/D2、T3/D3；T1/D1 由固件固定，T4=100/D4=100 固定）。
/// 采样策略：在用户曲线上按温度均分取 30%/60% 位置的点。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OemCurvePoints {
    pub t2: f64,
    pub d2: f64,
    pub t3: f64,
    pub d3: f64,
}

pub fn compress_to_oem_points(curve: &FanCurve) -> OemCurvePoints {
    let first = curve.points.first().copied().unwrap_or(CurvePoint { temp_c: 40.0, duty_pct: MIN_DUTY_PCT });
    let last = curve
        .points
        .last()
        .copied()
        .unwrap_or(CurvePoint { temp_c: 100.0, duty_pct: MAX_DUTY_PCT });
    let span = last.temp_c - first.temp_c;
    let sample_at = |ratio: f64| -> (f64, f64) {
        let temp = first.temp_c + span * ratio;
        (temp, curve.interpolate(temp))
    };
    let (t2, d2) = sample_at(0.3);
    let (t3, d3) = sample_at(0.6);
    OemCurvePoints {
        t2: t2.round(),
        d2: d2.round().clamp(MIN_DUTY_PCT, MAX_DUTY_PCT),
        t3: t3.round(),
        d3: d3.round().clamp(MIN_DUTY_PCT, MAX_DUTY_PCT),
    }
}

/// 单个风扇的固件固定端点（cmd13 读出）：T1/D1 由 BIOS 决定，T4/D4 = 100 固定。
#[derive(Clone, Copy, Debug)]
pub struct OemFanAnchors {
    pub t1: f64,
    pub d1: f64,
}

/// OEM cmd14 包构建所需的每风扇完整输入。
#[derive(Clone, Copy, Debug)]
pub struct OemFanCurveInput {
    pub anchors: OemFanAnchors,
    pub points: OemCurvePoints,
}

/// 曲率斜率 R = (D_next − D_cur) / (T_next − T_cur) × 2.55 × 16，取整。
/// 温差为 0 时返回 0（调用方校验已排除此情况）。
pub fn oem_slope(d_cur: f64, d_next: f64, t_cur: f64, t_next: f64) -> i32 {
    let dt = t_next - t_cur;
    if dt.abs() < f64::EPSILON {
        return 0;
    }
    ((d_next - d_cur) / dt * 2.55 * 16.0).round() as i32
}

/// 校验 OEM 三点曲线的几何约束。返回错误信息（空 = 通过）。
pub fn validate_oem_curve(input: &OemFanCurveInput) -> Result<(), String> {
    let (t1, d1) = (input.anchors.t1, input.anchors.d1);
    let p = &input.points;
    if !(p.t2 > t1 && p.t3 > p.t2 && 100.0 > p.t3) {
        return Err(format!(
            "温度必须严格递增：T1({t1}) < T2({p0}) < T3({p1}) < 100",
            p0 = p.t2,
            p1 = p.t3
        ));
    }
    if !(p.d2 > d1 && p.d3 > p.d2 && p.d3 < 100.0) {
        return Err(format!(
            "占空比必须严格递增且 <100：D1({d1}) < D2({d0}) < D3({d1v}) < 100",
            d0 = p.d2,
            d1v = p.d3
        ));
    }
    Ok(())
}

/// 构建 cmd14 的 256 字节写入包（布局契约 docs/oem-dchu-contract.md §5 cmd14）。
/// CPU 包体在 [2..13]，GPU1 在 [6..9]……按 OEM Write_WMI14 语义：
/// [2..5]=CPU T2/D2/T3/D3，[6..9]=GPU1，[10..13]=GPU2，
/// [14..19]=CPU 斜率，[20..25]=GPU1 斜率，[26..31]=GPU2 斜率（u16 LE）。
pub fn build_curve_package(
    cpu: &OemFanCurveInput,
    gpu1: &OemFanCurveInput,
    gpu2: &OemFanCurveInput,
) -> [u8; 256] {
    let mut buf = [0u8; 256];
    let mut put_points = |at: usize, p: &OemCurvePoints| {
        buf[at] = p.t2 as u8;
        buf[at + 1] = (p.d2 / 100.0 * 255.0).round() as u8;
        buf[at + 2] = p.t3 as u8;
        buf[at + 3] = (p.d3 / 100.0 * 255.0).round() as u8;
    };
    put_points(2, &cpu.points);
    put_points(6, &gpu1.points);
    put_points(10, &gpu2.points);

    let mut put_slopes = |at: usize, a: &OemFanCurveInput| {
        let (t1, d1) = (a.anchors.t1, a.anchors.d1);
        let p = &a.points;
        let slopes = [
            oem_slope(d1, p.d2, t1, p.t2),
            oem_slope(p.d2, p.d3, p.t2, p.t3),
            oem_slope(p.d3, MAX_DUTY_PCT, p.t3, 100.0),
        ];
        for (i, s) in slopes.iter().enumerate() {
            let bytes = (*s as u16).to_le_bytes();
            buf[at + i * 2] = bytes[0];
            buf[at + i * 2 + 1] = bytes[1];
        }
    };
    put_slopes(14, cpu);
    put_slopes(20, gpu1);
    put_slopes(26, gpu2);
    buf
}

/// 组装 page4 持久化扇区中某风扇（CPU/GPU1/GPU2）的可调点字段。
/// 返回 (duty 区间起点, temp 区间起点, slope 区间起点)，占位与 OEM SetFanInfo 一致：
/// CPU [16..33]、GPU1 [34..51]、GPU2 [52..69]。
pub fn oem_page4_offsets(fan_index: usize) -> (usize, usize, usize) {
    let base = 16 + fan_index * 18;
    (base, base + 6, base + 12)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve() -> FanCurve {
        FanCurve::new(
            vec![
                CurvePoint { temp_c: 40.0, duty_pct: 20.0 },
                CurvePoint { temp_c: 60.0, duty_pct: 50.0 },
                CurvePoint { temp_c: 80.0, duty_pct: 100.0 },
            ],
            2.0,
            10.0,
        )
    }

    #[test]
    fn interpolate_linear_segments() {
        let c = curve();
        assert_eq!(c.interpolate(40.0), 20.0);
        assert_eq!(c.interpolate(50.0), 35.0);
        assert_eq!(c.interpolate(60.0), 50.0);
        assert_eq!(c.interpolate(70.0), 75.0);
        assert_eq!(c.interpolate(90.0), 100.0);
        assert_eq!(c.interpolate(30.0), 20.0); // 低于首点取首点
    }

    #[test]
    fn new_clamps_duty_and_sorts() {
        let c = FanCurve::new(
            vec![
                CurvePoint { temp_c: 80.0, duty_pct: 100.0 },
                CurvePoint { temp_c: 40.0, duty_pct: 5.0 }, // 低于下限 → 20
            ],
            2.0,
            10.0,
        );
        assert_eq!(c.points[0].temp_c, 40.0);
        assert_eq!(c.points[0].duty_pct, 20.0);
        assert_eq!(c.points[1].temp_c, 80.0);
    }

    #[test]
    fn evaluate_ramp_limits_step() {
        let c = curve();
        // 目标 100，上次 20，限速 10%/s → 30
        assert_eq!(c.evaluate(90.0, 20.0, 1.0), 30.0);
        // dt 越小步进越小
        assert_eq!(c.evaluate(90.0, 20.0, 0.5), 25.0);
    }

    #[test]
    fn evaluate_thermal_fuse() {
        let c = curve();
        let fused = c.evaluate(96.0, 20.0, 100.0);
        // 熔断：目标 100；dt=100s 时斜坡不受限 → 100
        assert_eq!(fused, 100.0);
    }

    #[test]
    fn evaluate_hysteresis_holds_duty() {
        let c = curve();
        // 上次输出 50（对应 60°C 交点），温度降到 58.5（回落 < 2°C 滞后）→ 维持 50
        assert_eq!(c.evaluate(58.5, 50.0, 100.0), 50.0);
        // 温度回落超过滞后 → 允许下降
        let released = c.evaluate(57.0, 50.0, 100.0);
        assert!(released < 50.0);
    }

    #[test]
    fn evaluate_duty_floor() {
        let c = curve();
        // 任何情况输出都 ≥ 20
        assert_eq!(c.evaluate(20.0, 20.0, 100.0), 20.0);
    }

    #[test]
    fn compress_picks_reasonable_points() {
        let oem = compress_to_oem_points(&curve());
        // 曲线 40..80：30% 位置 52°C → 插值 20 + (12/20)*30 = 38%；
        // 60% 位置 64°C → 插值 50 + (4/20)*50 = 60%
        assert_eq!(oem.t2, 52.0);
        assert_eq!(oem.d2, 38.0);
        assert_eq!(oem.t3, 64.0);
        assert_eq!(oem.d3, 60.0);
    }

    fn oem_input(t1: f64, d1: f64, t2: f64, d2: f64, t3: f64, d3: f64) -> OemFanCurveInput {
        OemFanCurveInput {
            anchors: OemFanAnchors { t1, d1 },
            points: OemCurvePoints { t2, d2, t3, d3 },
        }
    }

    #[test]
    fn validate_rejects_non_increasing() {
        let ok = oem_input(40.0, 20.0, 55.0, 40.0, 70.0, 60.0);
        assert!(validate_oem_curve(&ok).is_ok());
        // T2 ≤ T1 拒绝
        assert!(validate_oem_curve(&oem_input(40.0, 20.0, 40.0, 40.0, 70.0, 60.0)).is_err());
        // D3 ≥ 100 拒绝（D4 固定 100，需严格递增）
        assert!(validate_oem_curve(&oem_input(40.0, 20.0, 55.0, 40.0, 70.0, 100.0)).is_err());
    }

    #[test]
    fn slope_matches_oem_formula() {
        // (50-20)/(60-40) * 2.55 * 16 = 61.2 → 61
        assert_eq!(oem_slope(20.0, 50.0, 40.0, 60.0), 61);
        assert_eq!(oem_slope(50.0, 50.0, 60.0, 60.0), 0);
    }

    #[test]
    fn build_package_layout() {
        let mk = |t1: f64, d1: f64, pts: OemCurvePoints| OemFanCurveInput {
            anchors: OemFanAnchors { t1, d1 },
            points: pts,
        };
        let cpu = mk(
            40.0,
            20.0,
            OemCurvePoints { t2: 55.0, d2: 40.0, t3: 70.0, d3: 60.0 },
        );
        let gpu1 = mk(
            45.0,
            25.0,
            OemCurvePoints { t2: 60.0, d2: 45.0, t3: 75.0, d3: 65.0 },
        );
        let gpu2 = gpu1;
        let buf = build_curve_package(&cpu, &gpu1, &gpu2);
        // CPU 点位
        assert_eq!(buf[2], 55);
        assert_eq!(buf[3], (40.0f64 / 100.0 * 255.0).round() as u8);
        assert_eq!(buf[4], 70);
        assert_eq!(buf[5], (60.0f64 / 100.0 * 255.0).round() as u8);
        // GPU1 点位
        assert_eq!(buf[6], 60);
        // CPU 斜率 R12 = (40-20)/(55-40)*2.55*16 = 54.4 → 54
        let r12 = u16::from_le_bytes([buf[14], buf[15]]);
        assert_eq!(r12, 54);
        // GPU1 斜率区起点 20
        let g_r12 = u16::from_le_bytes([buf[20], buf[21]]);
        assert_eq!(
            g_r12,
            ((45.0f64 - 25.0) / (60.0 - 45.0) * 2.55 * 16.0).round() as u16
        );
        // 其余区全零（GPU2 与 GPU1 相同 → 各自区域有值；远端 [32..] 为零）
        assert_eq!(buf[40], 0);
    }

    #[test]
    fn page4_offsets_layout() {
        let (d, t, s) = oem_page4_offsets(0);
        assert_eq!((d, t, s), (16, 22, 28));
        let (d, t, s) = oem_page4_offsets(1);
        assert_eq!((d, t, s), (34, 40, 46));
        let (d, t, s) = oem_page4_offsets(2);
        assert_eq!((d, t, s), (52, 58, 64));
    }
}
