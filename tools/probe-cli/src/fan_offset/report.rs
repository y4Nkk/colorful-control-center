use super::{
    Phase,
    experiment::{Row, Sample},
};
use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Clone, Copy, Debug)]
struct Summary {
    n: usize,
    duty: f64,
    rpm: f64,
    temp: f64,
    gpu_duty: f64,
    gpu_rpm: f64,
}

pub(super) struct Csv(std::fs::File);

impl Csv {
    pub(super) fn create(path: &str) -> Result<Self> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| format!("创建取样文件 {path} 失败（已存在时不会覆盖）"))?;
        writeln!(
            file,
            "t_s,phase,sampled,live_mode_bits,cpu_duty_pct,cpu_rpm,cpu_temp_c,gpu_duty_pct,gpu_rpm,gpu_temp_c"
        )?;
        Ok(Self(file))
    }

    pub(super) fn append(&mut self, row: &Row) -> Result<()> {
        let sample = &row.sample;
        writeln!(
            self.0,
            "{},{},{},{:#04x},{:.1},{:.0},{},{:.1},{:.0},{}",
            row.at,
            row.phase,
            u8::from(row.sampled),
            row.live,
            sample.cpu_duty,
            sample.cpu_rpm,
            sample.cpu_temp,
            sample.gpu_duty,
            sample.gpu_rpm,
            sample.gpu_temp
        )?;
        self.0.flush()?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Response {
    Clear,
    None,
    Partial,
}

/// 判定阈值为经验值：占空比抬高 5 个百分点或温度压低 2°C 才算响应。
fn response(x: Summary, base: Summary) -> Response {
    let (duty, temp) = (x.duty - base.duty, x.temp - base.temp);
    if duty >= 5.0 || temp <= -2.0 {
        Response::Clear
    } else if duty <= 1.5 && temp >= -1.0 {
        Response::None
    } else {
        Response::Partial
    }
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    match values.len() % 2 {
        0 => (values[mid - 1] + values[mid]) / 2.0,
        _ => values[mid],
    }
}

fn summarize(rows: &[Row], phase: &str) -> Option<Summary> {
    let picked: Vec<Sample> = rows
        .iter()
        .filter(|row| row.phase == phase && row.sampled)
        .map(|row| row.sample)
        .collect();
    if picked.len() < 10 {
        return None;
    }
    let stat = |pick: fn(Sample) -> f64| {
        let mut values: Vec<f64> = picked.iter().map(|sample| pick(*sample)).collect();
        median(&mut values)
    };
    Some(Summary {
        n: picked.len(),
        duty: stat(|sample| sample.cpu_duty),
        rpm: stat(|sample| sample.cpu_rpm),
        temp: stat(|sample| sample.cpu_temp),
        gpu_duty: stat(|sample| sample.gpu_duty),
        gpu_rpm: stat(|sample| sample.gpu_rpm),
    })
}

/// 夹住对照阶段的前后两个基线取平均，抵消温度的单向漂移。
fn average(a: Summary, b: Summary) -> Summary {
    let mean = |x: f64, y: f64| (x + y) / 2.0;
    Summary {
        n: a.n + b.n,
        duty: mean(a.duty, b.duty),
        rpm: mean(a.rpm, b.rpm),
        temp: mean(a.temp, b.temp),
        gpu_duty: mean(a.gpu_duty, b.gpu_duty),
        gpu_rpm: mean(a.gpu_rpm, b.gpu_rpm),
    }
}

pub(super) fn report(rows: &[Row], plan: &[Phase; 6]) {
    println!("\n阶段        样本  CPU占空比%  CPU转速  CPU温度°C  GPU占空比%  GPU转速");
    let stats =
        std::array::from_fn::<Option<Summary>, 6, _>(|index| summarize(rows, plan[index].name));
    for (phase, stat) in plan.iter().zip(&stats) {
        match stat {
            Some(stat) => println!(
                "{:<10} {:>4} {:>11.1} {:>8.0} {:>10.1} {:>11.1} {:>8.0}",
                phase.name, stat.n, stat.duty, stat.rpm, stat.temp, stat.gpu_duty, stat.gpu_rpm
            ),
            None => println!("{:<10} 数据不足", phase.name),
        }
    }
    let [auto0, auto_x, auto0b, quiet0, quiet_x, quiet0b] = stats;
    let (Some(auto0), Some(auto_x), Some(auto0b), Some(quiet0), Some(quiet_x), Some(quiet0b)) =
        (auto0, auto_x, auto0b, quiet0, quiet_x, quiet0b)
    else {
        println!("样本不足，未做判定。");
        return;
    };
    let (auto_base, quiet_base) = (average(auto0, auto0b), average(quiet0, quiet0b));
    for (label, base, x) in [
        ("自动档", auto_base, auto_x),
        ("寂静档", quiet_base, quiet_x),
    ] {
        println!(
            "{label}：偏移相对 0 → CPU 占空比 {:+.1} 个百分点，转速 {:+.0}，温度 {:+.1}°C；GPU 占空比 {:+.1}，转速 {:+.0}",
            x.duty - base.duty,
            x.rpm - base.rpm,
            x.temp - base.temp,
            x.gpu_duty - base.gpu_duty,
            x.gpu_rpm - base.gpu_rpm
        );
    }
    println!(
        "判定：{}",
        verdict(
            response(auto_x, auto_base),
            response(quiet_x, quiet_base),
            quiet_base.duty
        )
    );
    println!("结论仅适用于本次负载、电源模式与对照偏移。");
}

/// 本机寂静档在高温下两路占空比同为 0x8E（约 56%），疑似上限；基线贴近时偏移会被遮蔽。
const CAP_HINT: f64 = 54.0;

fn verdict(auto: Response, quiet: Response, quiet_base_duty: f64) -> &'static str {
    if auto != Response::Clear {
        return "无法判定：自动档对照组对偏移响应不足。请调整负载或对照偏移后重试。";
    }
    if quiet_base_duty >= CAP_HINT && quiet != Response::Clear {
        return "无法判定：寂静档基线占空比已接近高温下观察到的上限，偏移可能被上限遮蔽。请降低负载后重试。";
    }
    match quiet {
        Response::Clear => "寂静档生效，偏移在寂静档下同样有响应。",
        Response::None => "寂静档未生效，自动档有响应而寂静档的占空比与温度均未变化。",
        Response::Partial => "寂静档仅有部分响应，请结合上表与取样文件人工判断。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(duty: f64, temp: f64) -> Summary {
        Summary {
            n: 60,
            duty,
            rpm: duty * 80.0,
            temp,
            gpu_duty: duty,
            gpu_rpm: duty * 80.0,
        }
    }

    fn row(phase: &'static str, sampled: bool, duty: f64) -> Row {
        Row {
            at: 0,
            phase,
            sampled,
            live: 0x02,
            sample: Sample {
                cpu_duty: duty,
                cpu_rpm: duty * 80.0,
                cpu_temp: 70.0,
                gpu_duty: duty,
                gpu_rpm: 0.0,
                gpu_temp: 50.0,
            },
        }
    }

    #[test]
    fn response_needs_a_clear_duty_or_temperature_change() {
        let base = summary(35.0, 75.0);
        assert_eq!(response(summary(50.0, 75.0), base), Response::Clear);
        assert_eq!(response(summary(35.0, 72.0), base), Response::Clear);
        assert_eq!(response(summary(35.5, 74.5), base), Response::None);
        assert_eq!(response(summary(37.5, 74.5), base), Response::Partial);
    }

    #[test]
    fn verdict_needs_a_control_and_refuses_a_capped_baseline() {
        assert!(verdict(Response::Clear, Response::Clear, 20.0).starts_with("寂静档生效"));
        assert!(verdict(Response::Clear, Response::None, 20.0).starts_with("寂静档未生效"));
        assert!(verdict(Response::None, Response::None, 20.0).starts_with("无法判定：自动档"));
        assert!(verdict(Response::Clear, Response::None, 55.7).starts_with("无法判定：寂静档基线"));
        assert!(verdict(Response::Clear, Response::Clear, 55.7).starts_with("寂静档生效"));
    }

    #[test]
    fn summarize_ignores_settling_samples_and_outliers() {
        let mut rows: Vec<Row> = (0..20).map(|_| row("auto-0", false, 99.0)).collect();
        rows.extend((0..9).map(|_| row("auto-0", true, 40.0)));
        rows.push(row("auto-0", true, 95.0));
        rows.push(row("quiet-0", true, 10.0));
        let stat = summarize(&rows, "auto-0").unwrap();
        assert_eq!(stat.n, 10);
        assert!((stat.duty - 40.0).abs() < 1e-9);
        assert!((stat.rpm - 3200.0).abs() < 1e-9);
        assert!(summarize(&rows, "auto-x").is_none());
    }
}
