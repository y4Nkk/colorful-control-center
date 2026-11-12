# OEM InsydeDCHU 硬件接口契约（基准机逆向事实）

> 本文是本项目硬件通道事实的唯一所有者。来源：基准机（将星 X15 AT 23，BIOS INSYDE
> 1.07.16TCOLO2，Model `NP5x_6x_7x_SNx`）上 OEM Control Center v7.019 与
> ColorfulCenter CC3 的反编译审查（用户授权审查自有软件），交叉验证 nbfc-linux
> `Colorful X15 AT 22.json` 配置。本文记录的是接口事实（命令号、缓冲区布局），
> 不含任何第三方代码。

## 1. 通道结论

- **root\WMI 无任何 OEM 类**（tools/probe/wmi-enum-results.txt，2026-11-12 实测）。
  OEM 的通道不是 WMI，而是：
- **主通道：`InsydeDCHU.dll`（x64）+ 已安装的 `AcpiBridge.sys` 驱动**。
  DLL 位于 `C:\Program Files (x86)\ControlCenter\DCHU\InsydeDCHU.dll`，
  依赖 `AcpiBridge` 驱动（实测 Running）与 `CCDCHUService` 服务（实测 Running）。
  卸载 OEM Control Center 会移除该通道，届时 HAL 报告不可用并降级只读。
- **兜底通道：原始 EC 端口读写**（见 §8，NBFC 社区配置，标注未在本机验证）。
- runtime\WMI 也确认无 TongFang/Uniwill/Insyde 类。

## 2. P/Invoke 签名（DLL 全部导出，x64）

```c
int GetDCHU_Data_Integer(int command, int* data);            // 读整数
int GetDCHU_Data_Buffer(int command, byte* buffer);          // 读 256B 包
int SetDCHU_Data(int command, byte* buffer, int length);     // 写（长度 4 或 256）
int SetDCHU_DataEx(int command, byte* buffer, int length, byte* outBuffer); // 写并回读
int ReadAppSettings(int page, int offset, int length, byte* buffer);
int WriteAppSettings(int page, int offset, int length, byte* buffer);
```

包装惯例（OEM 代码同款语义）：
- `SetWMI(cmd, uint)` = SetDCHU_Data(cmd, 4B LE)
- `SetWMI(cmd, sub, uint)` = 4B LE，最高字节（buf[3]）= sub
- `SetWMIPackage(cmd, 256B)` = 整包写
- `GetWMIPackage(cmd)` = 256B 包读
- AppSettings 的 (page, offset) 在 OEM 代码中常写作 `index/256, index%256`
- 返回值：`0x80000002`（-2147483646）= 不支持；-1 = 失败
- ColorfulCenter CC3 的写入走命名管道 `CC3Helper` → helper 服务，但其
  WritePolicy 白名单为空（"EC writer is not enabled"）——CC3 只读不写。

## 3. 读命令表（GetDCHU_Data_Integer）

| cmd | 含义 |
|---|---|
| 3 | 电源模式（0=安静 1=省电 2=性能 3=娱乐） |
| 16 | CPU 频率 MHz（也是特性位图①：bit0 PowerMode 支持 / bit1 Flexikey / bit2 FlexiAccess / bit3 PerKeyKB / bit4 RGBKB / bit5 GPUOC / bit6 CPUOC / bit7 FanSetting）|
| 29 | CPU 温度 °C |
| 50 | CPU 最大频率 MHz |
| 85 | GPU 温度 °C |
| 96 | GPU 频率 MHz（特性位图②：bit7 AntiDust 风扇 / bit10 FanOffset；CPUOC 另查 bit23）|
| 101 | 风扇占空比（0-255） |
| 111 | 已用内存 KB |
| 70 | Init_WMI 触发（能力枚举入口） |
| 122 | bit15 FanLess；bit23 = CPUOC 支持位 |

## 4. 读包布局（GetDCHU_Data_Buffer）

### cmd 12 — 风扇实时状态

| 偏移 | 字段 |
|---|---|
| [2..3] | CPU 风扇 RPM（LE u16，高字节在后：buf[3]<<8\|buf[2]）|
| [16] | CPU 风扇 duty（0-255） |
| [18] | CPU remote 温度（原始值，需按 TDP 曲线换算，见 §9） |
| [4..5] / [19] / [21] | GPU1 风扇 RPM / duty / remote |
| [6..7] / [22] / [24] | GPU2 风扇 RPM / duty / remote |
| [36..37] / [38] / [40] | Sys 风扇 RPM / duty / remote |

### cmd 13 — 风扇能力与默认曲线

| 偏移 | 字段 |
|---|---|
| [12] | 风扇数量 FanCount |
| [14] | 初始风扇模式（5 = MaxQ） |
| [16..21] | CPU：T1, D1(0-255→%), T2, D2, T3, D3（T4=100, D4=100 固定）|
| [24..29] / [32..37] | GPU1 / GPU2 同构 |
| [43] | bit1 = 不支持自定义曲线 |
| [50] | bits_FanExist（bit0 CPU, bit1 GPU1, bit2 GPU2, bit3 Sys）|

### cmd 18 — 超频默认值包

| 偏移 | 字段 |
|---|---|
| [8..11] / [12..15] | PL1 / PL2 默认（u32 LE，自动判单位：>1e6 → µW，>1e3 → mW，否则 W）|
| [16..21] | 内存电压：unit / 当前 / 默认 |
| [22..45] | 内存支持电压列表（u16 LE ×12） |
| [48/80/112/144..] | 4 组 XMP 时序（tCL…，每组 32B） |
| [176..177] | CPU 核心电压偏移默认（u16，bit15=1 → 正偏移 (v^0x8000)，否则负偏移）|
| [180..181] | GT（核显）电压偏移默认（u16，值为负向） |
| [208..218] | 默认倍频（P 核 10 个 + E 核，u8）|
| [219..220] | PL 时间窗默认（u16 LE） |
| [221..224] | VR 默认（u32 LE，0xFFFFFFFF = VR Auto） |

## 5. 写命令表

### cmd 121（子命令式：SetWMI(121, sub, value)）

| sub | 含义 |
|---|---|
| 1 | 设置风扇模式（值 = 模式枚举，见 §7）|
| 14 | 风扇偏移（duty% → *255/100）|
| 25 | Turbo 风扇开关（bit6=on；bit7 由 AppSettings(1,32) 同步；写法 `2\|on<<6`）|
| 34 | 恢复风扇默认（值 1）|
| 40 | 除尘调度（value = day + resume<<7 + hour<<8）|
| 41 | 立即除尘（值 1）|
| 45 | （OEM 使用中，语义未确认——勿用）|

### cmd 118：SetWMI(118, 1, time) 同步系统时间给 EC
（second | minute<<6 | hour<<12 | dayOfWeek<<17）

### cmd 14 — 写自定义风扇曲线（SetWMIPackage(14, 256B)）

| 偏移 | 字段 |
|---|---|
| [2]/[3] | CPU T2 / D2（duty% ×255/100）|
| [4]/[5] | CPU T3 / D3 |
| [6..9] / [10..13] | GPU1 / GPU2 的 T2,D2,T3,D3 |
| [14..19] | CPU 斜率 R12,R23,R34（u16 LE）|
| [20..25] / [26..31] | GPU1 / GPU2 斜率 |
| 斜率公式 | R = round((D_next − D_cur) / (T_next − T_cur) × 2.55 × 16)|

写入后须把模式切为 Custom（SetWMI(121,1,6)）并同步 AppSettings page4（§6）。
T1/D1 与 T4/D4=100 由固件固定，不可调——OEM 自定义曲线是 3 可调点。

### cmd 4（SetWMIPackageEx 双向查询/设置协议）

256B 包：buf[0]=子功能号，[1]=读序号（0/1/2/3 分批），[6]=长度或校验值；
回包 o_buffer 携带数据。已确认的子功能：

| buf[0] | 含义 |
|---|---|
| 40 / 42 | 读 Fan1+Fan2 / Fan3+Fan4 的 EC 10 级 RPM 风扇表（见下）|
| 41 / 43 | 写 Fan1+Fan2 / Fan3+Fan4 表（表体放 buf[16..127] 与 buf[128..239]，每表 112B）|
| [1,0..3,0,0,0,222] | 分 4 批读 EC 版本 ASCII（o_buffer[1..5]）|
| [1,9,0,0,0,0,192] | 读热状态（o_buffer[2] bit4 = LoadStorm）|
| [1,11,0,0,0,0,192] | 读 Turbo 风扇最大 duty（o_buffer[2]）|
| [1,163,0,0,0,0,184] | 读除尘状态（o_buffer[1]）|

**EC 10 级 RPM 风扇表布局（每表 112B，RPM_FanControl 能力开启时使用）**：

| 偏移 | 字段 |
|---|---|
| [0] | 风扇 ID（自定义时 = 128+profileID / 144+ / 160+ / 176+ 按 Fan1-4）|
| [1..7] | open_fan_localTemp, shutdown_localTemp, open_fan_temp, close_fan_temp, temp_shutdown, temp_passive, temp_fanfull |
| [8..13] | fan_idle_RPM, FanMax_RPM, FanMax_Turbo_RPM（u16 BE 序：buf[hi]<<8\|lo）|
| [22..31] | 每级 temp_down（×10 级）|
| [32..41] | 每级 temp_up |
| [42..61] | 每级 rpm_down（u16 LE ×10）|
| [62..81] | 每级 rpm_up（u16 LE ×10）|
| [82..91] | 每级 duty_up |
| [92..101] | 每级 duty_down |
| [102..111] | 每级打包字节（高 4 位 time_down，低 4 位 time_up）|

OEM 默认 10 级曲线：temp = [35,40,45,50,60,65,70,80,90,100]，
duty(RPM%) = [0,30,40,45,50,60,70,80,90,100]。

## 6. AppSettings 持久化页

### page 4 — 风扇（index 4*256 起）

| index(page4 内偏移) | 字段 |
|---|---|
| [4] / [5] | 初始模式 / 当前模式 |
| [7] | FanOffset（%）|
| [8] | TurboFan 状态 |
| [9] | SlowFan（静音）状态 |
| [16..19] | CPU D1..D4（0-100 直存）|
| [20..21] | CPU D2/D3 默认 |
| [22..25] | CPU T1..T4 |
| [26..27] | CPU T2/T3 默认 |
| [28..33] | CPU R12,R23,R34（u16 LE）|
| [34..69] | GPU1（同构）、GPU2（同构，最大 [69]）|
| [80] / [81..85] | 除尘状态 / 调度 |

### page 6 — CPU 超频持久化（BIOS/服务重应用）

| offset | 字段 |
|---|---|
| 9(4B) / 13(8B) / 21(1B) / 22(1B) / 23(4B) | 监控：CPU 频率 / 电压(double) / 利用率 / 温度 / 内存时钟 |
| 31(4B) | FullScreen 标志（OEM UI 用）|
| 32(1B) | **IsSaved 标志**——写 1 后固件/服务应用 page6 的值 |
| 33(4B) | **PL1 当前值** |
| 37(4B) | **PL2 当前值** |
| 41(4B) | PL 时间窗（float）|
| 49(4B) | CPU VR 上限 |
| 53..60 (1B×8) | P 核倍频（XTU id 29,30,31,32,42,43,96,97）|
| 61(4B) | **CPU 核心电压偏移**（undervolt 值）|
| 65(4B) | 覆盖电压（float）|
| 69(4B) | GT 核显电压偏移 |
| 73(1B) | 电压模式 |
| 74(1B) | VR Auto |
| 75..76 (1B×2) | P 核倍频（XTU id 107,108）|
| 77..92 (1B×16) | E 核倍频（XTU id 4500..4515）|

### page 7 — BIOS 特性表（v0/v1）

offset[2..5] 版本号；v1 判定 (buf[0]<<8|buf[1]) == 256。
[17] 低 4 位 = 电源模式支持位（bit0 Quiet / bit1 PowerSaving / bit2 Performance /
bit3 Entertainment），高 4 位 = PowerModeUI_ID；[16] bit4 = TurboFan；
[20] bit0 SlowFan / bit3 SimpleGPUOC / bit4 RPM_FanControl / bit5..7 IFSC；
[21] bit0/1 IFSC 性能/娱乐支持。

## 7. 风扇模式枚举（Interface_Fan.FanMode）

0=Auto, 1=Max(全速), 2=Silent?, 3=?, 4=?, 5=MaxQ, 6=Custom, 7=?, 8=Noiseless(SlowFan),
9=IFSC。语义已确认：0 Auto / 1 Max / 5 MaxQ / 6 Custom / 8 SlowFan；其余档位本机
未必支持（能力位过滤）。

## 8. 兜底：原始 EC 端口寄存器（NBFC 社区，未在本机验证）

来自 nbfc-linux `Colorful X15 AT 22.json`（GPL-3，同模具上一代）：

| 寄存器 | 用途 |
|---|---|
| 0x2C | 风扇控制权：写 8 = 软件接管，写 5 = 还原 BIOS 自动 |
| 0xE7 | CPU 风扇 duty 写（有效范围 20-100）|
| 0xCF | CPU 风扇 duty 读 |
| 0xE8? | GPU 风扇 duty 写（推测，未确认）|

使用前提：PawnIO（含端口 IO 模块）或 WinRing0。**优先用主通道，本兜底仅当
InsydeDCHU 不可用且用户显式启用。**

## 9. CPU remote 温度换算（OEM FAN.CalCPUTemp）

cmd12 的 buf[18] 是 EC 内部 remote 值，按 TDP 分段换算 °C：

- 84W/88W：v≤60 → v−1；否则 (v−12)×0.33+44
- 65W：v≤50 → v−1；否则 (v−35)×0.41+43.7
- 91W：v≤50 → v−5；否则 (v−9)×0.22+43.7
- 35W：v≤32 → v；否则 v×0.5+16
- 47W：v≤26 → v；否则 v×0.5+13
- 其他：直通

## 10. Phase 3 桥接（XTU）

OEM 的 PL/undervolt 应用是双轨：
1. **持久化**：写 AppSettings page6 + IsSaved(1) → 固件/服务在合适的时机应用
   （本机 `XTU3SERVICE` 已安装，OEM CPU_OC 依赖 `IntelOverclockingSDK.dll`
   运行时 Tune(id,value)+ApplyChanges；XTU 控件 id 与 page6 偏移一一对应）。
2. **运行时**：本项目主走 MSR 直写（0x610 PL / 0x150 偏移），probe-cli 先测锁；
   MSR 被锁时降级 page6 持久化路径（需重启生效）并明确提示。

## 11. 已知未知项（写入实现前必须补验）

1. **电源模式固件写入命令**：CC3 只切 Windows 电源计划；固件级切换由 Fn 守护
   进程（native）完成，命令号未提取到。候选：SetDCHU_Data(3, mode) 同命令空间
   回写——启用前必须在真机验证。
2. GPU 风扇 EC 寄存器（0xE8 推测）。
3. page6 写入的生效时机（立即 vs 重启）与 XTU3SERVICE 的角色边界。
4. cmd 121 sub45 语义。

## 12. 本项目使用规则

- 一律经 Rust FFI 封装（`hal::oem_dchu`），禁止其他模块直接加载 DLL。
- 所有写入必须过 HAL 钳位（占空比 ≥20%、PL 上限、undervolt 上限 −100mV 起）。
- 模块接入点见 AGENTS.md；安全语义见 docs/design.md §4。
