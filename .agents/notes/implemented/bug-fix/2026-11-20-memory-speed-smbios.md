# Agent Note: 内存频率改用 SMBIOS 数据源

Status: implemented

## 问题

- 内存详情弹窗的频率事实只有「原厂报告频率/原厂参考频率」两行，数据来自 OEM 共享报告（`read_settings(6,23/230)`）。该通道未报告时（如真机当前状态）频率恒显示「未知」，用户无从得知实际内存速率。

## 决定

2026-11-20 内存频率改由本机 SMBIOS 提供，不再依赖 OEM 通道：

- `hal/smbios.rs` 从「只解析系统厂商」扩展为共享结构遍历 + Type 17（内存条）解析：已占用插槽（Size ∉ {0, 0xFFFF}）取配置速率（offset 0x20，需结构长度 ≥ 0x22），未报告时回退最大速率（offset 0x14）；多插槽取最高值。厂商解析行为不变（原有测试全保留）。
- SMBIOS 速率语义为 DDR 有效传输速率，字段名与显示单位用 MT/s，不与 OEM 行的 MHz 混用。
- `hal/overview.rs`（非 OEM 概览契约）的 `SystemOverview` 新增 `memory_speed_mts`，读取失败进 `warnings`、值为空，与容量读取同一模式。

### 契约

- `SystemOverview.memorySpeedMts: number | null`（serde camelCase）为内存频率唯一所有者；前端 `api/hardware.ts` 同名类型。
- `CapacityOverview.tsx` 内存卡统计区（可用/总容量旁）与详情弹窗各显示「频率/内存频率」（`formatParameter(…, "MT/s")`），弹窗中置于容量事实与原厂共享报告事实之间；原厂两行保留作共享报告对照。
- `MEMORY_HELP` 与 `DESIGN.md`「内存频率事实」行同步登记 SMBIOS 数据源与 MT/s 语义。

## 影响与验证

- `cargo check`、`npm run build`、`npm run verify-notes` 通过；SMBIOS 解析逻辑经独立编译夹具验证 8 项（占用/空槽/回退/缺省/截断/真实布局全通过）。`cargo test smbios` 当前被并行在途重构（powerctl/fanctl/profiles 测试文件）阻断，待其修复后随套件回归。
- 顺手修复在途 `hal/overview/cpu.rs` 中 `nominal_base_clock_mhz` 缺 `__cpuid` 导入导致的 lib 编译阻断（不修则任何改动的 `cargo check` 基线无法通过）。
- 真机显示效果待目检；本改动只读固件表，无硬件写入风险。

## 备选方案

- 在 OEM 共享报告通道内做兜底：该通道语义是"原厂报告"，混入本机 SMBIOS 会破坏其对照用途，且通道不可用时依旧无频率可显示——不采用。
- 走 WMI `Win32_PhysicalMemory`：需要 COM/WMI 运行时与异步查询，而 SMBIOS 原始表经 `GetSystemFirmwareTable` 同步可得且同源——不采用。
