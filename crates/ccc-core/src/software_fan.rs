//! 自由曲线契约、逐通道 C1 编码和有状态控制；不访问硬件。
use crate::fan::{FanCurve, oem_duty_byte};
use std::collections::BTreeMap;

pub const FAN_IDS: [&str; 4] = ["cpu", "gpu1", "gpu2", "sys"];
pub type Curves = BTreeMap<String, FanCurve>;

pub fn validate(curves: &Curves) -> Result<(), String> {
    if curves.is_empty() {
        return Err("软件曲线为空".into());
    }
    for (id, curve) in curves {
        if !FAN_IDS.contains(&id.as_str()) {
            return Err(format!("未知风扇 {id}"));
        }
        if !(2..=12).contains(&curve.points.len())
            || curve.hysteresis_c.fract() != 0.0
            || curve.ramp_pct_per_s.fract() != 0.0
            || !curve.hysteresis_c.is_finite()
            || !(0.0..=10.0).contains(&curve.hysteresis_c)
            || !curve.ramp_pct_per_s.is_finite()
            || !(1.0..=100.0).contains(&curve.ramp_pct_per_s)
            || curve.points.iter().any(|p| {
                p.temp_c.fract() != 0.0
                    || p.duty_pct.fract() != 0.0
                    || !p.temp_c.is_finite()
                    || !p.duty_pct.is_finite()
                    || !(20.0..=95.0).contains(&p.temp_c)
                    || !(20.0..=100.0).contains(&p.duty_pct)
            })
            || curve
                .points
                .windows(2)
                .any(|p| p[0].temp_c >= p[1].temp_c || p[0].duty_pct > p[1].duty_pct)
            || curve
                .points
                .last()
                .is_none_or(|p| p.temp_c != 95.0 || p.duty_pct != 100.0)
        {
            return Err(format!(
                "{id} 软件曲线无效：2–12 个递增温度点、20–100% 不降，末点 95°C/100%；回差 0–10°C，斜坡 1–100%/秒"
            ));
        }
    }
    Ok(())
}

pub fn duty_packet(channel: u8, percent: f64) -> Result<[u8; 256], String> {
    if !(1..=4).contains(&channel) || !percent.is_finite() || !(20.0..=100.0).contains(&percent) {
        return Err("风扇通道或风扇强度无效".into());
    }
    let mut packet = [0; 256];
    packet[0] = 1;
    packet[1] = channel;
    packet[2] = oem_duty_byte(percent);
    packet[6] = 0xc1;
    Ok(packet)
}

#[derive(Clone)]
pub struct Controller {
    pub curves: Curves,
    duties: BTreeMap<String, f64>,
}

impl Controller {
    pub fn new(curves: Curves, initial: &BTreeMap<String, f64>) -> Result<Self, String> {
        validate(&curves)?;
        let duties = curves
            .keys()
            .map(|id| {
                let duty = initial
                    .get(id)
                    .copied()
                    .filter(|d| d.is_finite())
                    .unwrap_or(100.0);
                (id.clone(), duty.clamp(20.0, 100.0))
            })
            .collect();
        Ok(Self { curves, duties })
    }

    pub fn step(&mut self, temperature: f64, dt_s: f64) -> Result<Vec<[u8; 256]>, String> {
        if !temperature.is_finite()
            || !(0.0..95.0).contains(&temperature)
            || !dt_s.is_finite()
            || !(0.0..=3.0).contains(&dt_s)
        {
            return Err("温度失效、达到 95°C 或软件调度超时，须恢复自动控制".into());
        }
        let mut packets = Vec::new();
        for (id, curve) in &self.curves {
            let next = curve.evaluate(temperature, self.duties[id], dt_s);
            let channel = FAN_IDS.iter().position(|known| known == id).unwrap() as u8 + 1;
            packets.push(duty_packet(channel, next)?);
            self.duties.insert(id.clone(), next);
        }
        Ok(packets)
    }
}

#[cfg(test)]
#[path = "software_fan/tests.rs"]
mod tests;
