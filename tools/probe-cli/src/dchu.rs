//! 探测只读外观；DLL ABI 唯一实现复用桌面 HAL，不公开写入方法。
#[allow(dead_code)]
#[path = "../../../src-tauri/src/hal/oem_dchu.rs"]
mod transport;
use anyhow::{Result, bail};

pub struct Dchu(transport::Dchu);
impl Dchu {
    pub fn bios_capabilities(&self) -> Result<[u8; 256]> {
        self.0.bios_capabilities()
    }
    pub fn load() -> Result<Self> {
        transport::Dchu::load().map(Self)
    }
    pub fn read_int(&self, command: i32) -> (bool, i32) {
        self.0.read_int(command)
    }
    pub fn read_package(&self, command: i32) -> Result<[u8; 256]> {
        self.0.read_package(command)
    }
    pub fn read_settings(&self, page: i32, offset: i32, length: usize) -> Result<Vec<u8>> {
        self.0.read_settings(page, offset, length)
    }
    /// 仅放行风扇运行时命令（121/1 档位、121/14 偏移）；不写 AppSettings。
    pub fn set_fan_runtime(&self, sub: u8, value: u32) -> Result<()> {
        if !matches!(sub, 1 | 14) || value > 0xFF {
            bail!("探测只允许风扇档位与偏移命令");
        }
        self.0.set_data_sub(121, sub, value)
    }
}

fn u16_le(buf: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([buf[at], buf[at + 1]])
}
fn u32_le(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}

/// 能力位图①（cmd 16）解码。
pub fn decode_cmd16(v: i32) -> Vec<&'static str> {
    let v = v as u32;
    let mut out = vec![];
    for (bit, name) in [
        (0, "PowerMode"),
        (1, "Flexikey"),
        (2, "FlexiAccess"),
        (3, "PerKeyKB"),
        (4, "RGBKB"),
        (5, "GPUOC"),
        (6, "CPUOC"),
        (7, "FanSetting"),
    ] {
        if (v >> bit) & 1 == 1 {
            out.push(name);
        }
    }
    out
}

/// cmd12/13 包十六进制转储：区分「整数读回显」与「包读正常」。
pub fn dump_packages() -> Result<()> {
    let dchu = Dchu::load()?;
    for cmd in [12u32, 13u32] {
        let pkg = dchu.read_package(cmd as i32)?;
        println!("== cmd{cmd} 包（256B，非零行） ==");
        let mut any = false;
        for row in 0..16 {
            let line: Vec<String> = (0..16)
                .map(|c| format!("{:02x}", pkg[row * 16 + c]))
                .collect();
            let nonzero = pkg[row * 16..row * 16 + 16].iter().any(|&b| b != 0);
            if nonzero {
                any = true;
                println!("  {:02x}: {}", row * 16, line.join(" "));
            }
        }
        if !any {
            println!("  （全零）");
        }
    }
    Ok(())
}
pub fn report() -> Result<()> {
    let dchu = Dchu::load()?;
    println!("== InsydeDCHU 能力报告（只读）==");

    let (ok16, cmd16) = dchu.read_int(16);
    println!(
        "cmd16 特性位图: {:?} -> {}",
        cmd16,
        if ok16 {
            decode_cmd16(cmd16).join(", ")
        } else {
            "读取失败".to_string()
        }
    );

    let (ok122, cmd122) = dchu.read_int(122);
    let cpuoc_wmi122 = ok122 && (cmd122 as u32 >> 23) & 1 == 1;
    println!(
        "cmd122: bit15 FanLess={} bit23 CPUOC={}",
        ok122 && ((cmd122 as u32 >> 15) & 1) == 1,
        cpuoc_wmi122
    );

    let (ok96, cmd96) = dchu.read_int(96);
    println!(
        "cmd96: bit7 AntiDust={} bit10 FanOffset={}",
        ok96 && ((cmd96 as u32 >> 7) & 1) == 1,
        ok96 && ((cmd96 as u32 >> 10) & 1) == 1
    );

    // BIOS 特性实时查询（原厂page7缓存的来源，不读取缓存）
    let page7 = dchu.bios_capabilities()?;
    let version = (u16::from(page7[0]) << 8) | u16::from(page7[1]);
    if version >= 256 {
        let mode_bits = page7[17];
        println!(
            "BIOS 特性表 v1: Quiet={} PowerSaving={} Performance={} Entertainment={}",
            mode_bits & 1 == 1,
            (mode_bits >> 1) & 1 == 1,
            (mode_bits >> 2) & 1 == 1,
            (mode_bits >> 3) & 1 == 1
        );
    } else {
        println!("BIOS 特性表 v0: 电源模式支持位 @offset17 = {}", page7[17]);
    }

    println!(
        "RPM 风扇表能力位：{}",
        version == 256 && page7[20] & 0x10 != 0
    );
    print_defaults(&dchu)?;
    println!("PL/降压实际可写性未验证；能力位和保存值不能证明即时参数可写。");
    println!("MSR 锁位可显式运行 pl；电压写入测试需 uv-test --execute，测试会恢复原编码。");
    Ok(())
}

fn print_defaults(dchu: &Dchu) -> Result<()> {
    let p18 = dchu.read_package(18)?;
    let pl1_raw = u32_le(&p18, 8);
    let pl2_raw = u32_le(&p18, 12);
    let scale = |raw: u32| -> (u32, &'static str) {
        if raw > 1_000_000 {
            (raw / 1_000_000, "W(µW)")
        } else if raw > 1_000 {
            (raw / 1_000, "W(mW)")
        } else {
            (raw, "W")
        }
    };
    let (pl1, u1) = scale(pl1_raw);
    let (pl2, u2) = scale(pl2_raw);
    let uv_raw = u16_le(&p18, 176);
    let uv_default = if (uv_raw >> 15) & 1 == 1 {
        (uv_raw ^ 0x8000) as i32
    } else {
        -(uv_raw as i32)
    };
    let gt_raw = u16_le(&p18, 180);
    println!(
        "WMI18 默认: PL1={} {} PL2={} {} CPU电压偏移默认={}mV GT偏移默认={}(u16) PL时间窗={} VR={:#x}",
        pl1,
        u1,
        pl2,
        u2,
        uv_default,
        gt_raw,
        u16_le(&p18, 219),
        u32_le(&p18, 221)
    );

    Ok(())
}
