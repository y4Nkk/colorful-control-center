use super::model::*;
use anyhow::{Context, Result, bail};
use std::path::Path;

pub fn parse(
    mut get: impl FnMut(&str, &str) -> Result<String>,
    cap: &Capability,
) -> Result<Option<Policy>> {
    let count: usize = get("MainSetting", "Count")?
        .parse()
        .context("原厂节能计划数量无效")?;
    if count == 0 {
        return Ok(None);
    }
    let index: usize = get("MainSetting", "SelectProfile")?
        .parse()
        .context("原厂节能选择无效")?;
    if count == 0 || count > 256 || index >= count {
        bail!("原厂节能选择超出计划范围");
    }
    let section = format!("profile_{index}");
    let boolean = |value: String| -> Result<bool> {
        match value.to_ascii_lowercase().as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => bail!("原厂节能布尔值无效"),
        }
    };
    let enabled = boolean(get("MainSetting", "EnergySaveStart")?)?;
    let every_day = boolean(get(&section, "Everyday")?)?;
    let automatic = boolean(get(&section, "AUTO")?)?;
    let mut weekdays = 0;
    for (bit, name) in ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"]
        .into_iter()
        .enumerate()
    {
        if boolean(get(&section, name)?)? {
            weekdays |= 1 << bit;
        }
    }
    let mut slots = [None, None];
    for (i, slot) in slots.iter_mut().enumerate() {
        let name = format!("Schedule{}", i + 1);
        if boolean(get(&section, &format!("{name}_Enable"))?)? {
            let start = get(&section, &format!("{name}_Start"))?
                .parse()
                .context("原厂时段起点无效")?;
            let end: u8 = get(&section, &format!("{name}_End"))?
                .parse()
                .context("原厂时段终点无效")?;
            *slot = Some(Slot {
                start,
                end: end.checked_add(1).context("原厂时段终点越界")?,
            });
        }
    }
    let limits = if automatic {
        cap.factory.clone()
    } else {
        Limits {
            floor: get(&section, "DischargeLimit")?
                .parse()
                .context("原厂放电阈值无效")?,
            ceiling: get(&section, "ChargeLimit")?
                .parse()
                .context("原厂充电阈值无效")?,
        }
    };
    let policy = Policy {
        enabled,
        every_day,
        weekdays,
        slots,
        automatic,
        limits,
    };
    policy.validate(cap)?;
    Ok(Some(policy))
}
pub fn read(path: &Path, cap: &Capability) -> Result<Option<Policy>> {
    use windows_sys::Win32::System::WindowsProgramming::GetPrivateProfileStringW;
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => (),
        Ok(_) => bail!("原厂节能路径不是普通文件"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("原厂节能文件不可读取"),
    }
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain([0]).collect() };
    let file = wide(path.to_str().context("原厂节能路径无效")?);
    parse(
        |section, key| {
            let (section, key) = (wide(section), wide(key));
            let mut buffer = [0u16; 1024];
            let length = unsafe {
                GetPrivateProfileStringW(
                    section.as_ptr(),
                    key.as_ptr(),
                    std::ptr::null(),
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    file.as_ptr(),
                )
            } as usize;
            if length == 0 || length >= buffer.len() - 1 {
                bail!("原厂节能字段缺失或过长");
            }
            String::from_utf16(&buffer[..length]).context("原厂节能文本无效")
        },
        cap,
    )
}
