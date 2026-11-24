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
- **逐通道控制：cmd4/EC C1**（见 §8）：本机方法体静态确认后已接入软件闭环，物理行为未验收。NBFC 原始端口配置仅作历史线索，不作为回退。
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
- ColorfulCenter CC3 的写入走命名管道 `CC3Helper` → helper 服务，但其
  WritePolicy 白名单为空（"EC writer is not enabled"）——CC3 只读不写。

本机实际使用的原生 DLL SHA256 为 `D385DEF14C5AA6CBB8EB60FD34D0EFEB92EAF39E875AB28DE05FE0FDAA7CAB9C`。
AppX 中同名 DLL 的哈希不同，不能混用返回约定。静态反汇编核对的返回值如下（RVA 相对 DLL ImageBase）：

| 导出 / RVA | 本版本返回语义 |
|---|---|
| GetDCHU_Data_Integer / 0x2680 | 成功返回整数负载且写到 data；传输失败返回 0、不写 data。当前平台回声值仍不作为传感或能力依据 |
| SetDCHU_Data / 0x28A0 | 返回固件整数结果；缓冲结果返回命令号。使用的 4/14/39/94/103/121 及已静态确认的 0x69 分支均以命令号确认传输，0 是失败 |
| SetDCHU_DataEx / 0x2AE0 | 返回命令号不能单独证明成功，IOCTL 失败可能未填输出；还须检查输出及领域回读 |
| GetDCHU_Data_Buffer / 0x2D50 | 正常返回命令号，传输失败返回 0 |
| ReadAppSettings / 0x2F80 | 正常返回整个存储长度 4096，失败返回 0；只向调用者复制请求长度 |
| WriteAppSettings / 0x3090 | 正常返回 1，失败返回 0；保存字段仍必须回读 |

GetBuffer/SetDataEx 在缓冲负载后额外复制首字节。只分配 256B 会越界；HAL 分配 1025B 内部缓冲，领域只消费 256B，并检查第 256 字节与首字节相同，拒绝 Ex 未填输出。AppSettings 仅有 16 页（0–15），单次访问不跨 256B 页界。
能力来源必须区分查询与缓存：FnKey的Features.BIOSSpecialFeature先以SetWMIPackageEx(4,首字节8)取得256B能力包，再写入AppSettings page7。自研由hal/oem_dchu/capabilities唯一执行该只读语义的查询，不依赖原厂预先初始化缓存，也不回写共享page7。直接消费者使用read_bios_capabilities/bios_capabilities；SettingsAccess的逻辑page7视图复用同一实时包，read_settings拒绝把page7当保存事实。失败、空输出和未知版本明确报错，不回退旧缓存或按机型填默认能力。模式表无显式候选时，按原厂WMI16 bit0决定基础四模式；1.0能力字段非零时读取包内值，原厂整数分支不可读时报告未知，不伪装未支持。原厂界面的 DTT 标记来自 WMI96 bit12，已核对固件使用「性能选项与寂静模式」一节的命令门控和驱动状态。

命令返回只证明传输/固件分支完成，不证明 EC 执行或物理效果；例如 cmd121 在 EC 不可用时可返回 Ones，HAL 会拒绝。不得同时把 0 与命令号当成功。

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

## 4. 读包布局（GetDCHU_Data_Buffer）—— ✅ 2026-11-12 真机验证为可靠通道

> 关键事实：`GetDCHU_Data_Integer` 存在回声缺陷（见 §11-0），**全部传感数据
> 必须走本节包读**。历史 cmd12 负载差分记录的是测速计数器变化（474↔473），换算为 4549↔4559 RPM，
> CPU remote buf[18] 活值（82→84 随负载上升）。

### cmd 12 — 风扇实时状态（✅ 已验证）

| 偏移 | 字段 |
|---|---|
| [2..3] | CPU 测速周期计数器（u16 BE），原始 474 对应 4549 RPM，不能直接标作 474 RPM |
| [16] | CPU 风扇 duty（0-255） |
| [18] | **CPU remote 温度（本版本 GetTDP 返回 0，温度直通，见 §9）** |
| [4..5] / [19] / [21] | GPU1 测速计数器 / duty / remote（本机双风扇：CPU+GPU1）|
| [6..7] / [22] / [24] | GPU2 测速计数器 / duty / remote（本机不存在）|
| [36..37] / [38] / [40] | Sys 测速计数器 / duty / remote |

`Page_system_monitor.UpdateUI_CPUFan/UpdateUI_GPUFan` 的实际显示公式为：计数器 0→0 RPM，否则 `Math.Round(60 / (5.565217391304348E-05 * counter) * 2)`（中点取偶数）。桌面采样和核心测试复用同一公式。十级表里的 RPM 字段本身就是 RPM，不做此周期换算。

### cmd 13 — 风扇能力与默认曲线（结构已填充；字段偏移待逐项校准）

| 偏移 | 字段 |
|---|---|
| [12] | 风扇数量 FanCount（本机实测 2）|
| [14] | EC 命令 0xD7 的回读值，即运行时档位位图（见 §7）；原厂代码把它当「初始风扇模式」读。本机 0x40 = 寂静位，与 `page4[5]=8` 吻合（2026-11-24 只读核对）|
| [16..21] | CPU：T1=51, D1≈22.7%, T2=60, D2≈32%, T3=80, D3≈33%（本机实测值）|
| [24..29] / [32..37] | GPU1 / GPU2 同构（本机 GPU1 区含 0xFF 占位——偏移待校准）|
| [43] | bit1 = 不支持自定义曲线 |
| [50] | bits_FanExist（bit0 CPU, bit1 GPU1, bit2 GPU2, bit3 Sys）|

### cmd 18 — 超频默认值包（✅ 本机实测：PL1/PL2 默认 135W/135W）

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
| 1 | 设置风扇模式（值 = 模式枚举，见 §7）；AML 发 EC 命令 0xD7 写档位位图，见 §8 |
| 14 | 风扇偏移（duty% → *255/100）；原厂在自动档及寂静覆盖档暴露，见 §11-9。AML 直接写 EC 字段 `GFOF`，见 §8 |
| 25 | OEM 电源模式（低六位 0=安静/1=省电/2=性能/3=娱乐）。性能档和经核对的娱乐档可合并 bit6 Turbo 与 bit7 DTT，见下 |
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
| [14..19] | CPU 斜率 R12,R23,R34（u16 BE；与 page4 保存格式相反）|
| [20..25] / [26..31] | GPU1 / GPU2 斜率 |
| 斜率公式 | R = round((D_next − D_cur) / (T_next − T_cur) × 2.55 × 16)|

写入后须把模式切为 Custom（SetWMI(121,1,6)）并同步 AppSettings page4（§6）。
T1/D1 与 T4/D4=100 由固件固定，不可调——OEM 自定义曲线有两个可调点。

### cmd121/sub25 — OEM 电源模式

Pi 会话后续反编译的 `dec/fnkeynet/FnKey.exe_dec/FnKey/Features.cs::SetPowerMode`
明确给出实际写入路径：page1[1] 保存模式，cmd121/sub25 执行切换。
性能模式的命令值为 `2 | (page4[8]==1 ? 0x40 : 0) | (page1[32]==1 ? 0x80 : 0)`；
原版对 0/1/3 直接发送模式编号，并按 DTT 驱动检测清除失效标志。本项目按能力验证与驱动校验应用已选的标志。模式 4（Quiet Plus）保存独立编号 4，即时命令映射到 0。

应用按 page7[17] 的低四位开放模式；仅扩展页头 `[1,0]` 解释 bit7，以模式 4 替代 0。
旧页头 `[0,0]` 不推断扩展标志，未知页头拒绝写入。联动策略同样仅从扩展能力读取：

- page7[20] & 0xD0 或 page7[21] & 3 声明分模式风扇记忆；page4[70..74] 保存模式、[74..78] 保存偏移，0/4 共用槽0。全速记忆为自动。娱乐慢风扇策略可即时执行8而仍保存0。
- page7[16] & 8 声明 ASPM；cmd4 子功能15的字节1/2、3/4、5/6、7/8依次为性能、娱乐、安静、省电的 AC/DC 值。Quiet Plus 使用娱乐槽。
- page7[19] & 2 声明 Windows 计划；cmd4 子功能29的字节1、2、3、4依次为性能、娱乐、安静、省电，值0/1/2为平衡/高性能/节能。Quiet Plus 使用安静槽。

本机 FnKey 7.19.5.0 的 `SetPowerMode` 和 `Load_FanMode_forFn_3` 直接发送固件模式、风扇档与偏移，执行前不查询实时温度。FanSpeedSetting 6.91 的 `Set_FanMode`、`SetFanOffset`、`Write_WMI14` 和 RPM 表保存同样直接交给固件。
预检捕获模式、完整风扇状态、原活动计划 GUID 及将修改的 ASPM 值；首次基线先于写入。
按原厂顺序修改原活动计划 ASPM，再激活目标计划。控制台与手动档案均绑定已预览联动目标，变化后拒绝目标写入。
失败独立尝试模式、系统策略及风扇恢复，逐项保留错误并允许重试。代码所有者为 `powerctl/mode/`，Windows API 由 `hal/power_plan.rs` 拥有。
page1 回读只确认保存状态，不是固件执行结果；物理效果仍须验收。独显关断与屏幕刷新率不在本事务内。

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
| [42..61] | 每级 rpm_down（u16 BE ×10）|
| [62..81] | 每级 rpm_up（u16 BE ×10）|
| [82..91] | 每级 duty_up |
| [92..101] | 每级 duty_down |
| [102..111] | 每级打包字节（高 4 位 time_down，低 4 位 time_up）|

OEM 默认 10 级曲线：temp = [35,40,45,50,60,65,70,80,90,100]，
duty(RPM%) = [0,30,40,45,50,60,70,80,90,100]。

此分支由 page7 v1 的 [20] bit4（RPM_FanControl）选择，读取时第一表在
回包 [0..111]、第二表在 [128..239]，与写包第一表 [16..127] 的位置不同。
原版 `Interface_FanTable.AnalyzFanTable/Fan_to_Buffer` 均使用大端 RPM。
当前应用按能力选择两点或十级 RPM 分支，后者通过 SetDCHU_DataEx(cmd4) 实现 40/41/42/43。只有 cmd13[50] 的存在位确认的风扇可编辑，四把分别使用各自最大 RPM；未修改伴随表及 [1..21]/[82..111] 保留。
目标 ID 使用当前 page1[1] 模式：0/4→profile0，1→1，2→2，3→3；每把加 0x80/0x90/0xA0/0xB0。新目标温度严格递增、RPM 百分比 20–100 单调不降、末级 100%，采用 .NET Math.Round 中点取偶数。down[0]=0、down[i]=up[i−1]+1，up/down RPM 同目标。
每次写后重新读整对表比较，失败恢复精确原始字节及模式/保存字段。默认恢复使用上列 OEM 固定曲线，响应参数 duty_up=26、duty_down=10、time_down/up=6/1，完成后进入自动模式；普通手动目标不能借此绕过 20% 下限。新表行为及响应时间单位未经真机确认，不对原厂 XML 写入。
运行时表事务、图形/数值编辑和档案已接入，字节及故障测试为隔离内存传输，不表示真实固件验收完成。
能效优先（MaxQ）仅在 `cmd13[14] == 5` 时开放。寂静模式（Noiseless）由 `page7[20]` bit0 声明。

## 6. AppSettings 持久化页

### page 4 — 风扇（index 4*256 起）

| index(page4 内偏移) | 字段 |
|---|---|
| [4] / [5] | 初始模式 / 当前模式 |
| [7] | FanOffset（%）|
| [8] | TurboFan 状态 |
| [9] | 娱乐模式的寂静偏好（SlowFan，0/1） |
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
| 9(4B) / 13(8B) / 21(1B) / 22(1B) / 23(4B) | CPUOC读取的共享报告：CPU频率 / 电压(double) / 利用率 / 温度 / 内存时钟；CPU相关字段的发布者和更新时间尚未确认 |
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
| 230(2B) | 原厂内存参考频率（小端无符号，CPUOC 的 Memory_Speed_Max） |

内存监控沿用 CPUOC.Model.DCHU.GetRAMClock 的 page6[23..27] 小端有符号整数及 IndexViewModel.Initial 的 page6[230..232] 参考值。hal/memory.rs 唯一解码，hal/monitor.rs 并入当前 HardwareSnapshot.oemMemory；只有本次 cmd12 传输成功才读取，两字段独立失败保留另一项。正值按原厂 MHz 报告，零为未报告，负频率无效并保留诊断；参考值不是硬上限，不截断读数或换算 DDR 有效传输速率。共享报告可能延迟，传输成功不能证明它等于当前物理时钟；断连不保留上次值。读取不写 AppSettings。

### page 7 — BIOS 特性表（v0/v1）

页头[0..1]为版本；v1判定(buf[0]<<8|buf[1]) == 256。[2..5]为WMI82、[6..9]为WMI122、[13..14]为WMI16能力位图，均为小端。
[17] 低 4 位 = 电源模式支持位（bit0 Quiet / bit1 PowerSaving / bit2 Performance /
bit3 Entertainment），高 4 位 = PowerModeUI_ID；[16] bit4 = TurboFan；
[20] bit0 SlowFan / bit3 SimpleGPUOC / bit4 RPM_FanControl / bit5..7 IFSC；
[21] bit0/1 IFSC 性能/娱乐支持。

## 7. 风扇模式枚举（Interface_Fan.FanMode）

0=Auto, 1=Max(全速), 2=Silent?, 3=?, 4=?, 5=MaxQ, 6=Custom, 7=?, 8=Noiseless(SlowFan),
9=IFSC。语义已确认：0 Auto / 1 Max / 5 MaxQ / 6 Custom / 8 SlowFan；其余档位本机
未必支持（能力位过滤）。

`DCHU.SCMD` 0x79/0x01 把档位翻译成 EC 命令 0xD7 的位图：自动 `0x02`、全速 `0x10`、
静音 `0x08`、MaxQ `0x01`、自定义 `0x04`、档位 7 `0x20`、寂静 `0x40`；档位 3、4 不发命令。
cmd13[14] 回读的就是这个位图。

### 性能选项与寂静模式

`powerctl/mode/boost.rs` 拥有性能选项。原厂界面根据 page7 版本 1.0 的 byte16 bit4 显示 Turbo 风扇，依据 bytes10–11 的 WMI96 bit12 显示 DTT。未核对命令的固件沿用这两个标记。已核对的固件直接按实时命令门控判定。未知版本拒绝，旧版的整数需要真实响应。Turbo 风扇对应 EC 散热策略。

Pi中的原厂`FnKey.Features.SetPowerMode`读取page4[8]、page1[32]，性能模式发送cmd121/sub25的`2 | turbo<<6 | dtt<<7`；`CC40.PageSystemElement.Interface_Fan.Set_TruboFan`保存这两个字段并联动自动风扇，PageSystem在自定义风扇入口处理Turbo冲突。研究目录的实验性cc3不作为原厂协议证据。自研UI分别编辑两个已声明位，统一预览确认，不复制原厂UI将两个偏好耦合的行为。

`hal/firmware.rs` 匹配的 INSYDE ADL DSDT 中，DCHU `SCMD(0x79)` 的子命令 `0x19` 在 `PSF4 & 4` 下接收低六位的 0–3。bit6 选择 EC D8 Turbo 策略，bit7 和 `DPTF` 控制 `DTTF`，当前档使用低六位的值。实时 `PK04(8)` 响应 byte10 bit2 对应该命令门控。门控有效时，娱乐档发送 `3 | turbo<<6 | dtt<<7`，性能档使用低位 2。单独修改选项保留当前档和 Windows 电源计划。缺少娱乐档命令能力时，原厂界面已声明的偏好仍可保存。

已核对的固件在激活或重发 DTT 位前，检查 ACPI 路径 `\_SB.IETM` 的设备已启动、驱动已加载且无设备问题。核对的 `DptfTabl` 仅在 `DPTF=1` 且 `IN34=1` 时通过 `_STA` 公开该设备。基准机只读核对的两个界面标记均为零，实时命令门控和 `IETM` 驱动状态均有效。其他固件沿用原厂的 Dynamic Tuning 驱动识别。检测失败保留诊断，已有开启值仍可关闭。娱乐档从自定义固件曲线开启 Turbo 时确认交接到自动风扇，恢复沿用原曲线快照。单独修改 DTT 沿用当前风扇策略。

固件 `page7[20]` bit0 声明寂静档能力。娱乐模式的开关读写 `page4[9]`，启用发送 `cmd121/sub1=8`，关闭发送自动档 `0`，两种操作都将基础档位及可用的娱乐槽记为自动。该字段的首次修改前值随同模式风扇快照恢复。性能模式复用固件档位，支持分模式记忆时写入性能槽 `page4[72]`，其余设备只在当前性能模式生效。已激活的 Turbo 与寂静档互斥，重新打开 Turbo 时先退出寂静档。候选修订绑定当前模式、当前会话确认的命令、两项性能偏好、档位、寂静偏好以及驱动状态。

娱乐与性能均可重新应用已保存的寂静模式。切换自定义固件曲线时，`apply_oem_performance_options` 的 `replaceCustomFan` 绑定已确认的风扇来源与 `expectedRevision`。前端确认后提交显式寂静或娱乐档 Turbo 目标，后端核对自定义档和目标，再执行交接。风扇快照支持初始恢复及写入失败补偿。单独修改 DTT 保留当前的寂静策略。

`CoreSnapshot` 同时捕获原始 Turbo/DTT 保存值和恢复命令。娱乐档依据当前会话成功发送的命令记录激活位，待应用的偏好单独保存。选项、模式及档案的失败恢复共用首次原厂模式快照，某步失败仍继续其他恢复并保留重试入口。命令接受与保存值回读只能验证命令和设置结果，真机效应需单独核对。

档案 `performanceOptions` 保存能力绑定的 Turbo/DTT 偏好，并按当前模式的能力决定是否激活。明确指定模式时，所选偏好合入首次模式命令，`preview_profile_mode` 产生同一组合目标供手动确认。启动选择展示当前组合目标，保存只绑定文件，下次启动重新预检。

仅选项档案不改Windows计划，Turbo变化才联动自动风扇；活动Turbo与自定义/软件曲线冲突在全量预检中拒绝。软件会话需要临时释放时，按最终档案风扇目标继续，未指定目标则恢复原曲线。执行前复查完整风扇与Windows来源，拒绝将变化后的状态归入旧初始快照；无变化选项不读取无关系统计划。局部补偿成功不重复外层恢复，后续失败独立尝试原始选项及风扇，模式命令后再恢复即时CPU。

`read_oem_performance_options / apply_oem_performance_options`通过api/power.ts接入。模式和性能选项写入失败时由各自事务补偿。161项档案检查（含19项新增选项）、8项假监护进程和19项相关页面检查通过。驱动及监护工厂必须显式注入，未注入不回退生产设备或进程；未执行原生驱动识别或硬件操作。

## 8. 逐通道 EC 控制与自动恢复（本机固件静态确认）

只读复制本机 DSDT（Windows GetSystemFirmwareTable），用官方 ACPICA iASL 20260408 静态反编译。表长 551584B，OEM INSYDE/ADL，SHA256 `d5162a4a002576ad9dcc77115001fbc66801927d7890c1316b9839a3c14b5203`。未执行 AML 方法、未加载表、未调用 OEM DLL 或写硬件。iASL 的外部引用警告不影响以下完整的本地方法体核对。

FnKey `DCHU.cs::RW_ECCMD` 将命令放在包 [6]，数据放在 [1..5]，包 [0]=1。目标 DSDT 中 `\\_SB.DCHU.CC30` 将 cmd4 分派到 PK04；PK04 的 Case(1)、R007=0 分支将 [6]→EC FCMD、[1]→FDAT、[2]→FBUF，余下数据依次传递。`EC.ECMD` 使用 PATM 锁串行化命令。

| 操作 | 参数 |
|---|---|
| 单风扇 duty | SetDCHU_Data(4, 256B)：[0]=1、[1]=通道 1–4、[2]=round_even(percent/100*255)、[6]=0xC1，其余 0 |
| 恢复通道自动 | SetDCHU_Data(0x69, 4B LE mask)：bit0–3 对应通道 1–4；SCMD 逐通道执行 C1 + 0xFF |

手动 EC 参数是 `C1(channel, duty)`，自动恢复是 `C1(0xFF, channel)`，参数顺序不同；手动 100% 的 duty=255 不等同于恢复自动。应用仅对已核对的本机 CPU/GPU1 发出手动输出，恢复掩码为 3，由 hal/firmware.rs 唯一拥有。

目标反编译文件中 CC30 在 105266 行，PK04 转发在 105400–105424 行，ECMD 在 103543 行，SCMD 0x69 在 109585–109624 行。SCMD 0x68 会同时覆盖四通道，不能用于单通道更新。此路径不依赖猜测 GPU 0xE8；静态命令支持也不证明原厂后台不会覆盖目标或物理风扇已按预期动作。

协议线索来自 [ClevoFanControl 的控制实现](https://github.com/XiaoQing235/ClevoFanControl/blob/main/src/hardware.rs)，随后以本机 FnKey、DLL 与上述 DSDT 独立核对，未将其他版本当作本机实验证据。软件调度、图形/数值编辑、档案、监护与恢复已接入。仅 DSDT 哈希匹配并确认 CPU/GPU1 存在时开放；监护子进程也须核对恢复能力后才 ARM。隔离测试通过不表示物理接管或强杀后的真实散热已经验收。

**风扇档位与偏移的落点（2026-11-24 静态核对）**：EC RAM 经内存窗口 `0xFE0B0100` 映射，
其中 `DUT1`/`DUT2`（0xCE/0xCF）为两路指令占空比，`RPM1`/`RPM2`（0xD0/0xD2）为测速周期，
`DTHL`（0xD7，与档位命令同地址）为档位位图，热区 TZ0 另把 `PSV - DTHL` 当作回差使用。
`SetWMI(121, 14, …)` 不经 EC 命令，由 `SCMD` 0x79/0x0E 直接写 `GFOF`（0xE7）。`GFOF`
全表只有 `\_SB.WMI` 与 `\_SB.DCHU` 两处写入，AML 从不读取，切档也不清除；偏移是否叠加进
占空比完全由 EC 固件决定。自定义曲线表在窗口 `0xFE0B0300` 的 `FANQ`、`F1T1…F3R3`
（0x8A–0xB7），由命令 14 的 `PK0E` 写入。反编译产物在仓库外的 `F:\projects\cc-research\dsdt\`。

### NBFC 历史原始端口线索（未在本机验证）

来自 nbfc-linux `Colorful X15 AT 22.json`（GPL-3，同模具上一代）：

| 寄存器 | 用途 |
|---|---|
| 0x2C | 风扇控制权：写 8 = 软件接管，写 5 = 还原 BIOS 自动 |
| 0xE7 | CPU 风扇 duty 写（有效范围 20-100）|
| 0xCF | CPU 风扇 duty 读 |
| 0xE8? | GPU 风扇 duty 写（推测，未确认）|

该原始端口路径仅为研究线索，当前应用未实现原始端口接管或 WinRing0 回退。GPU 原始寄存器未确认，禁止按推测值写入；它已不构成使用上述逐通道 C1 协议的前提。

## 9. CPU remote 温度换算（OEM FAN.CalCPUTemp）

本次审查的 `FAN.cs::GetTDP()` 固定返回字符串 `"0"`，所以
`CalCPUTemp(GetTDP(), buf[18])` 走直通分支。应用与监护均使用此原始温度，
不再误套 65W 换算。与 HWiNFO 的准确性对齐仍待真机验收。

该函数保留的其他历史分支如下，只作协议记录，本版本不使用：

- 84W/88W：v≤60 → v−1；否则 (v−12)×0.33+44
- 65W：v≤50 → v−1；否则 (v−35)×0.41+43.7
- 91W：v≤50 → v−5；否则 (v−9)×0.22+43.7
- 35W：v≤32 → v；否则 v×0.5+16
- 47W：v≤26 → v；否则 v×0.5+13
- 其他：直通

占空比字节采用 OEM 的 `Math.Round(percent / 100 * 255)`（中点取偶数），
例如 30%→76、50%→128、70%→178；风扇偏移及曲线包共用同一转换。

## 10. Phase 3 桥接（XTU）

OEM 的 PL/undervolt 应用是双轨：
1. **持久化**：写 AppSettings page6 + IsSaved(1) → 固件/服务在合适的时机应用
   （本机 `XTU3SERVICE` 已安装，OEM CPU_OC 依赖 `IntelOverclockingSDK.dll`
   运行时 Tune(id,value)+ApplyChanges；XTU 控件 id 与 page6 偏移一一对应）。
2. **运行时**：本项目通过 MSR（0x610 PL / 0x150 偏移）写入并回读；
   与 page6 保存值分别展示。page6 的消费时机仍待验收，不能承诺重启后必然生效。

性能档案的顶层 PL/电压字段保持 page6 含义，独立 `runtimeCpu` 表示即时目标。其 pl1W/pl2W 保留 RAPL 读数精度，coreOffsetUnits/gtOffsetUnits 是有符号 1/1024 V 编码单位（−51→−49.8046875 mV），由 powerctl/runtime.rs 校验。档案应用前读取锁及请求域原始编码，模式和保存值写入后才执行即时目标；失败在恢复模式之后恢复 MSR，避免模式覆盖 CPU 快照。控制台也使用整组 CPU 事务，不再分开提交 PL 与电压。

## 11. 已知未知项（写入实现前必须补验）

0. **`GetDCHU_Data_Integer` 回声缺陷（✅ 管理员下复现，已定论）**：cmd29/85/101
   返回值恒等于命令号、负载下不变、管理员下同样复现——该导出在本平台不可用。
   **包读（GetDCHU_Data_Buffer）完全正常**（cmd12 真机负载差分验证）。全部
   传感路径已改走包读；CC3.exe 为 asInvoker 却能工作的机制仍未解释
   （推断其显示同样受此缺陷影响或经服务通道），不影响本项目。
   原厂138号白光事件的GetWMI(61)是一次性反馈读取，独立校验整数返回状态、未写输出、回声及0–5范围后才同步保存值；失败不以旧page2值代替。本机61号读取未验收，不能由其他命令的历史结果推断其成功，亦不恢复为持续传感通道。屏显样式位只决定提示范围，见keyboard-contract.md §17。
1. **电源模式**：FnKey 托管实现已确认 cmd121/sub25，见 §5；控制台、档案与恢复已接入。
   模式4、分模式风扇、ASPM及系统计划已按声明能力接入；独显与刷新率由设置页独立确认，不自动跟随模式，见 oem-device-contract.md。
   早先仅查看原生启动器所得的“命令未知”结论已失效，当前链路尚未真机验收。
2. 原始 GPU EC 寄存器仍未知；逐通道 C1 软件闭环已由本机方法体确认并接入，物理效果尚待验收（§8）。
3. page6 写入的生效时机（立即 vs 重启）与 XTU3SERVICE 的角色边界。
4. cmd 121 sub45 语义。
5. cmd16 双用途怪癖：返回当前 CPU 频率值，OEM 代码按位解读为特性支持位——
   本项目不将其作为能力依据；CPUOC 能力以 cmd122 bit23 与 WMI18 包为准。
6. **键盘 RGB/白光**：已按 kbled 接入 cmd103 九灯效、三区颜色/开关与 cmd39 亮度，
   page2 事务和恢复包含原始灯效。6/22 采用 kbled 6.26 编码；CC40 分支存在差异，
   详见 [keyboard-contract.md](keyboard-contract.md)。逐键通过独立 HID/MCU 路径接入，全部即时效果尚未真机验收。
7. **PawnIO 探测结论修订**：旧设备打开代码构造的 UTF-16 路径包含内部零字符，
   `CreateFileW=2` 不能证明设备未创建或驱动僵死，也不能推出必须重启。
   桌面与 CLI 已复用 ccc-core 的正确路径、RAPL 单位和 Mailbox 校验；本轮未真机复测，
   本机 MSR 可用性及固件锁状态仍未知。page6 回读只证明设置保存，不等于 PL/降压生效。
8. **负偏移探测语义**：显式 uv-test --execute 只向下调整一原始编码单位后回读并恢复。只有目标为负且恢复一致才能报告本次负偏移写入成功；正值下降但仍非负不证明降压，编码已到 −1024 时不写。探测事务与 5 项故障/边界测试由 ccc-core 拥有，本轮没有实际执行 Mailbox 命令。
9. **风扇加速偏移的档位与能力门控**：原厂「寂静模式」是娱乐模式下的自动档覆盖
   开关（`page4[9]`，运行时 `sub1=8`，基础档保持 0），不是独立风扇档；该开关不隐藏
   偏移滑杆，偏移提交只发 `sub14`，不改 `page4[5]`/`[9]`，FnKey 恢复时先下发已保存
   偏移再发 `sub1=8`。全速/MaxQ/自定义不提供偏移；独立的「静音」单选档（风扇枚举 2，
   受 FanLess 能力门控）隐藏偏移。WMI96 bit10=1（PSF 1.0 读 bytes10..11 能力字，
   旧页头活读 GetWMI(96)）或 RPM_FanControl 机型整体禁用并强制清零。本项目的独立风扇档
   保存 `page4[5]=8`，娱乐档性能选项使用 `page4[5]=0` 和 `page4[9]` 覆盖。面板和恢复基线
   共用有效档位，Turbo 生效时保留自动档及寂静偏好。手动偏移仅在固件能力已确认且处于自动或寂静档时开放。
   后端持写锁复核有效档位、记忆槽、能力、软件接管与除尘状态。保存失败时，以本次写入前的保存值
   重新下发 `sub14` 并报告补偿结果。原厂预设按模式事务执行联动偏移。
   2026-11-24 静态核对确认 AML 层没有互锁（§8），是否生效只取决于 EC 固件；原厂 FnKey
   恢复时先发偏移再发档位，Fn 键循环回自动且寂静偏好开启时只补发 `sub1=8`，`page4[5]=8`
   时原厂设置页显示自动单选并保留偏移滑杆。同日只读观察：本机寂静档、保存偏移 40%，
   重载下 CPU 98–99°C、GPU 93–94°C 时两路占空比同为 `0x8E`（55.6%，约 4650 RPM），与
   11-12 的存档读数一致。高温下偏移看不出效果由此成立，但原因未定：可能是寂静档把占空比
   封顶在该值，也可能是寂静曲线本身更低而偏移已经叠加。批探测
   `run-admin-probe.ps1 -ExecuteFanOffsetAb` 用同负载下偏移 0/X 的自动档与寂静档对照来区分，
   写入路径尚未在真机执行；重启恢复同样待验。

## 12. 本项目使用规则

- 一律经 Rust FFI 封装（`hal::oem_dchu`），禁止其他模块直接加载 DLL。
- 所有写入先校验：占空比 ≥20%、PL 35–157W 整数且 PL1≤PL2、核心 −150…+50mV、GT −150…0mV；失败返回原文并尝试恢复。
- 模块接入点见 AGENTS.md；安全语义见 docs/design.md §4。

## 13. 原厂风扇除尘

来源为本机FanSpeedSetting的InsydeDriver.BIOSSpecialFeature_v1_0、FAN.RunAntiDustMode/SetAntiDust_Scheduler/Read_AntiDust_Status和Page_system_monitor。page7版本1.0 bytes10..11的WMI96 bit7声明能力，不将其他WMI位图或整数回声用作依据。

- 实际状态：cmd4双向256B包首7字节为`[1,163,0,0,0,0,184]`，response[1]仅接受0未运行/1运行。page4[80]只是原厂UI缓存；原版读取后写零存在错误，本项目读取不写任何缓存。
- 一次性启动：cmd121/sub41值1。原厂仅在非全速且CPU/GPU占空比均不超过50%时允许；本项目还核对软件接管与校准互斥，执行前重新读取。传输失败也消费本次确认，不自动重发。成功只表示请求已发送，观察到1再到0才表达已结束。
- 原厂AntiDust_Done只清保存缓存，没有已确认的停止命令。因此没有sub41值0的猜测写入，也不显示虚构取消、总时长或百分比。
- 计划：cmd121/sub40值`weekdays | onResume<<7 | hour<<8`，周日bit0至周六bit6，hour为0..23；保存page4[81..85]为`[1,weekdays|onResume<<7,hour,0,0]`。启用前cmd118/sub1同步本地时钟。时钟在计划命令之前失败时，不修改计划或执行不必要的计划补偿。
- 首次基线保留原始5字节，写后只核对所属字段；失败分别尝试固件计划及原字节补偿，保留可重试基线和诊断。原保存标记0表示没有已报告计划，不等于已知EC关闭；恢复该初态会发关闭触发并还原原字节，界面明确不保证重建未公开的旧EC计划。普通启动不复制原厂自动开启每日/唤醒除尘的行为。
- 普通风扇、模式和档案执行/恢复在实际运行时拒绝；预检在其他域写入之前完成，已准备的模式执行也重新检查。软件闭环只在原厂能力未声明除尘或计划已明确关闭时开放。监护和温度熔断仍可交还自动控制。

IPC为`read_fan_dust / start_fan_dust / apply_fan_dust_schedule / restore_fan_dust_schedule`，由`api/fans.ts`封装。档案`dustSchedule`复用原事务与首次基线；关闭计划在软件曲线接管前，启用计划在明确退出软件控制后。预检检查最终组合，实际执行再次核对能力/保存字节/运行状态；恢复先后同样保留软件控制互斥。未报告计划时不伪造捕获，原始5B仍可在执行失败后精确恢复，未公开EC计划的限制不变。

15项领域、10项新增档案除尘、7项原除尘及6项新增档案页面检查通过，相关档案回归见验收文档；未运行完整测试套件。未加载原厂DLL或访问实际EC，物理清灰效果及固件定时/唤醒行为未验收。

## 14. 原厂屏幕唤醒请求

FnKey 215号非OLED分支由DCHUService启动`WakePanelUp_Hook.exe`；该辅助程序首次鼠标活动时调用`SetWMI(94, 1)`，首次键盘按下只退出。静态反汇编本机DLL的`SetDCHU_Data`返回约定，并对照本机DSDT `SCMD`的`0x5E`分支：输入非零时构造带`0xC2`的EC请求，分支返回`0x5E`。该结论只证明固件分支和传输约定，不证明物理屏幕已唤醒。

自研只在独立授权、面板及固件能力复核通过后发送4字节小端值1；不假设值0能撤销。完整来源、会话所有权和恢复语义见[keyboard-contract.md §20](keyboard-contract.md)。

## 15. 原厂 GPU Turbo 来源字段

FnKey 7.19.5.0 的 `GPU_Turbo_off` / `Load_GPU_forFn_3` 由事件96/97触发。`cmd4/sub9` 响应的 bytes2–3、4–5、6–7、8–9、10–11 依次是娱乐、性能、省电、安静和温度限制 MHz，小端16位；不把原厂辅助程序写入的 page5 缓存当成实时基础时钟。

`cmd4/sub8` 的0.0版 byte13 bit0、1.0版 byte15 bit0声明锁频能力；全模式偏移取 WMI96 bit5。0.0版读取有效的WMI96整数；1.0版按原厂将 WMI82、WMI122、WMI96和WMI16四项相加，为零时才使用整数回退，否则取包内 bytes10–11。整数返回无效或为已知回声时不猜能力。未知包版本拒绝接管。

`page1[1]` 是原厂模式；`page5[16..23]` 是前两张 GPU 保存的核心/显存 MHz 偏移，每16位数的高位为1时按原厂减65535，而不是普通补码。能力、目标表、保存偏移及完整设备身份绑定长期授权；模式和供电在事件发生时重新读取，并在每个写入阶段复核。实际 NVAPI 结构、逐域恢复和验证边界见 [NVAPI 契约 §9](nvapi-contract.md)。

## 16. 原厂模式 EnergyStar

FnKey `OnChangePowerMode` 在WMI122 bit30未声明硬件P-state时发送WMI79：安静/娱乐为2、省电/安静增强为1、性能为0。公开名称保持原始固件值，不为0/1/2猜测功耗或频率含义。

本机已保存且哈希匹配的DSDT `DCHU.SCMD` 将WMI79的0–2写入PSF1 bits4–5并执行AC.ADJP，`PK04/sub8` 1.0版在bytes2–5报告PSF1、bytes6–9报告WMI122。硬件P-state为已确认的不发送79状态；其他固件、不能回读的版本和原值3拒绝写入，不能用模式号或保存设置代替实时前态。1.0能力总和为空时的整数读取与原厂一致。

单项、热键、档案和自动供电模式通过公共事务，在模式/GPU调频之后、模式后独显与系统计划/风扇之前执行一次；确认目标和首次恢复都包含节能状态，成功需要回读。选项独立操作没有节能所有权。完整首次恢复先核对管理能力再输出，失败补偿独立尝试其余所属域并保留错误与恢复基线。包回读证明状态位，不证明实际CPU电源效果；验证仅用静态固件和注入传输。
