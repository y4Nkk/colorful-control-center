//! ccc-probe — Phase 0 硬件探测 CLI。
//! 子命令：
//!   dchu                  InsydeDCHU 能力报告（无管理员，纯只读）
//!   info                  CPUID 信息（无管理员）
//!   status                PawnIO 驱动安装状态
//!   install               部署 PawnIO 驱动（需管理员，一次性）
//!   pl                    读 0x610/0x606 解码 PL1/PL2/锁位（需管理员）
//!   uv-test [--execute]   0x150 降压锁检测：读当前偏移；--execute 时降低一编码单位
//!                         验证后还原（需管理员）
//!   msr-read <addr>       读任意 MSR（需管理员）
//!   fan-offset-ab         风扇加速偏移的寂静档对照：默认只读预览当前档位与偏移；
//!                         --execute 才切换档位与偏移，结束后恢复（经 run-admin-probe.ps1）
//!   charge                充电保护阈值的真机验收：默认只读核对当前范围与候选；
//!                         --execute 才写入测试范围，回读后恢复原值并复核

mod charge;
mod dchu;
mod fan_offset;
mod msr;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ccc-probe", about = "Colorful Control Center Phase 0 硬件探测")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// InsydeDCHU 能力报告（无管理员，纯只读）
    Dchu,
    /// InsydeDCHU cmd12/13 包转储（风扇表结构取证）
    DchuPkg,
    /// CPUID 信息
    Info,
    /// PawnIO 驱动安装状态
    Status,
    /// 部署 PawnIO 驱动（需管理员，一次性）
    Install,
    /// PL1/PL2 与锁位检测（需管理员）
    Pl,
    /// MSR 0x150 降压锁检测（需管理员；--execute 才会写入，验证后还原）
    UvTest(#[command(flatten)] UvArgs),
    /// 风扇加速偏移的寂静档对照（默认只读；--execute 才会切换档位与偏移，结束后恢复）
    FanOffsetAb(#[command(flatten)] fan_offset::FanOffsetArgs),
    /// 充电保护阈值的真机验收（默认只读；--execute 才写入测试范围并恢复原值）
    Charge(#[command(flatten)] charge::ChargeArgs),
    /// 读任意 MSR（需管理员）
    MsrRead {
        /// MSR 地址（支持 0x 前缀）
        addr: String,
        /// CPU 序号
        #[arg(long, default_value_t = 0)]
        cpu: u32,
    },
}

#[derive(Args)]
struct UvArgs {
    /// 实际降低一个电压编码单位，回读后精确还原
    #[arg(long)]
    execute: bool,
    /// 目标域：cpu|gpu|cache|uncore|analogio
    #[arg(long, default_value = "cpu")]
    plane: String,
}

fn parse_hex(s: &str) -> Result<u32> {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    Ok(u32::from_str_radix(s, 16)?)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Dchu => dchu::report(),
        Command::DchuPkg => dchu::dump_packages(),
        Command::Info => {
            print_cpuid();
            Ok(())
        }
        Command::Status => {
            let installed =
                msr_driver_rs::MsrDriver::is_installed().context("查询 PawnIO 服务状态失败")?;
            println!(
                "PawnIO 驱动: {}",
                if installed { "已安装" } else { "未安装" }
            );
            Ok(())
        }
        Command::Install => {
            msr_driver_rs::MsrDriver::install()
                .context("PawnIO 部署失败（需管理员运行；内部含签名 PawnIO.sys 部署）")?;
            println!("PawnIO 驱动已部署并启动");
            Ok(())
        }
        Command::Pl => cmd_pl(),
        Command::UvTest(args) => cmd_uv_test(args),
        Command::FanOffsetAb(args) => fan_offset::run(&args),
        Command::Charge(args) => charge::run(&args),
        Command::MsrRead { addr, cpu } => {
            let addr = parse_hex(&addr)?;
            let driver = msr_driver_rs::MsrDriver::new()
                .context("打开 PawnIO 失败：先以管理员运行 `ccc-probe install`")?;
            let value = driver.read_msr(addr, cpu).context("MSR 读取失败")?;
            println!("MSR {addr:#06x} (cpu{cpu}) = {value:#018x} ({value})");
            Ok(())
        }
    }
}

fn print_cpuid() {
    // 安全的 CPUID 读取（x86_64 内联汇编，无驱动依赖；rbx 由 asm 内部 push/pop 保存）
    #[cfg(target_arch = "x86_64")]
    unsafe fn cpuid_count(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
        let (a, ebx_out, c, d);
        // rbx 在 LLVM 内部保留，用 push/pop 保存后经通用寄存器引出（std 同款模式）
        unsafe {
            core::arch::asm!(
                "push rbx",
                "cpuid",
                "mov {0:e}, ebx",
                "pop rbx",
                out(reg) ebx_out,
                inout("eax") leaf => a,
                inout("ecx") sub => c,
                out("edx") d,
            );
        }
        (a, ebx_out, c, d)
    }

    #[cfg(target_arch = "x86_64")]
    fn u32_to_str(v: u32) -> String {
        v.to_le_bytes().iter().map(|&c| c as char).collect()
    }

    #[cfg(target_arch = "x86_64")]
    unsafe {
        let (max_leaf, b, c, d) = cpuid_count(0, 0);
        let _ = max_leaf;
        let vendor = format!("{}{}{}", u32_to_str(b), u32_to_str(d), u32_to_str(c));
        println!("vendor: {vendor}");
        let (a, _, _, _) = cpuid_count(1, 0);
        println!(
            "family/stepping: family={:x} model={:x} stepping={:x}",
            ((a >> 8) & 0xF) | (((a >> 20) & 0xF) << 4),
            ((a >> 4) & 0xF) | (((a >> 16) & 0xF) << 4),
            a & 0xF
        );
        let mut brand = String::new();
        for leaf in [0x8000_0002u32, 0x8000_0003, 0x8000_0004] {
            let (a, b, c, d) = cpuid_count(leaf, 0);
            brand.push_str(&u32_to_str(a));
            brand.push_str(&u32_to_str(b));
            brand.push_str(&u32_to_str(c));
            brand.push_str(&u32_to_str(d));
        }
        println!("brand: {}", brand.trim_matches(' '));
    }

    #[cfg(not(target_arch = "x86_64"))]
    println!("cpuid: 仅支持 x86_64");
}

fn cmd_pl() -> Result<()> {
    let io = msr::PawnIo::open_with_intel_msr()?;
    let pl = io.read_msr(0x610).context("读 0x610 失败")?;
    let rapl = io.read_msr(0x606).context("读 RAPL 单位 0x606 失败")?;
    println!(
        "MSR_PKG_POWER_LIMIT (0x610): {}",
        msr::decode_pkg_power_limit(pl, rapl)
    );
    println!();
    println!("结论：");
    let pl_raw = pl;
    let lock63 = (pl_raw >> 63) & 1 == 1;
    println!(
        "- PL 锁(bit63) {}：{}",
        if lock63 { "已锁定" } else { "未锁定" },
        if lock63 {
            "MSR 直写 PL1/PL2 已锁；OEM page6 仅能保存意图，实际生效未验证"
        } else {
            "锁位未设置；实际可写性仍需显式写入、回读和恢复验证"
        }
    );
    Ok(())
}

fn cmd_uv_test(args: UvArgs) -> Result<()> {
    let plane = msr::PLANES
        .iter()
        .find(|(n, _)| *n == args.plane)
        .map(|(_, p)| *p)
        .context("未知 plane，可选 cpu|gpu|cache|uncore|analogio")?;
    let plane_name = args.plane.clone();
    let io = msr::PawnIo::open_with_intel_msr()?;

    let current = msr::read_voltage_offset(&io, plane).context("读取当前偏移失败")?;
    println!("plane {plane_name} 当前电压偏移: {current:+.3} mV");

    if !args.execute {
        println!();
        println!("这是只读探测（读协议本身经 mailbox 写命令）。");
        println!("写入锁检测需 --execute：降低一编码步进（约 0.98 mV）→ 回读 → 精确还原。");
        println!("此操作会改变硬件电压；尚未执行，因此不能判断降压写入是否可行。");
        return Ok(());
    }

    let probe = msr::test_voltage_write(&io, plane)?;
    let tested = f64::from(probe.target_units) / 1.024;
    let restored = f64::from(probe.before_units) / 1.024;
    println!("已确认写入 {tested:+.3} mV，并精确恢复 {restored:+.3} mV");
    if probe.negative_offset_verified() {
        println!("结论：本次负偏移写入及恢复均通过回读校验；稳定性未验证");
    } else {
        println!("结论：本次降低一步及恢复通过；目标仍非负，尚未验证负偏移写入");
    }
    Ok(())
}
