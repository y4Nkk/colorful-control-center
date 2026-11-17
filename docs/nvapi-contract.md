# NVAPI GPU 频率偏移与原厂锁频契约

> 本文件是 GPU 核心/显存时钟偏移及原厂 GPU Turbo 锁频通道的 ABI 与安全语义唯一事实所有者。
> 实现位置：`src-tauri/src/hal/nvapi.rs`（会话）、`hal/nvapi/ffi.rs`（ABI）、
> `hal/nvapi/pstates.rs`（结构视图）、`hal/nvapi/transaction.rs`（事务）、
> `src-tauri/src/gpuctl.rs`（控制器）。硬件写入安全总纲见 `docs/design.md` §4。

## 1. 通道与加载

- 通过 `LoadLibraryExA(LOAD_LIBRARY_SEARCH_SYSTEM32)` 只从系统目录加载
  `nvapi64.dll`（x64 规范名）→ `nvapi.dll` 兜底，杜绝 CWD/PATH DLL 劫持。
  本机 System32 实测仅存在 `nvapi64.dll`（7,133,024 字节）。
- 入口导出 `nvapi_QueryInterface`，按 ID 解析函数指针；**解析失败即报错返回**，
  不猜测、不静默降级、不伪造成功。
- 会话惰性建立：`GpuController::new()` 不触碰硬件，只有运行时命令
  （read/apply/restore）才加载 DLL 并调用 `NvAPI_Initialize`。
- `NvAPI_Initialize`/`NvAPI_Unload` 成对调用（官方契约）；会话由
  `GpuController` 的单一 `Mutex` 串行化（同时满足串行写入要求）。

## 2. 接口 ID（全部经多源交叉验证）

| 函数 | ID | 验证来源 |
|---|---|---|
| `NvAPI_Initialize` | `0x0150E828` | NvAPIWrapper FunctionId.cs:707、bemanitools module.c:749、NVFC、ccminer nvapi.cpp:36 |
| `NvAPI_Unload` | `0xD22BDD7E` | NvAPIWrapper:706、bemanitools:750、NVFC nvapi.cpp:249 |
| `NvAPI_GetErrorMessage` | `0x6C2D048C` | NvAPIWrapper:163 |
| `NvAPI_EnumPhysicalGPUs` | `0xE5AC921F` | NvAPIWrapper:155、bemanitools:164、NVFC:251 |
| `NvAPI_GPU_GetFullName` | `0xCEEE8E9F` | NvAPIWrapper:205、NVFC nvapi.h:321 |
| `NvAPI_GPU_GetPstates20` | `0x6FF81213` | 官方 nvapi.h:5240（有函数声明）、NvAPIWrapper:216、bemanitools:225、NVFC:258、jNizM NvAPI_IDs.txt |
| `NvAPI_GPU_SetPstates20` | `0x0F4DAE6B` | NvAPIWrapper:622、bemanitools:627、NVFC:259、ccminer:359、jNizM、NtMiner、bemanitools 等 10+ 独立实现 |

**已证伪的流传值**：`0x08595393`（原 zcode 猜测）与 `0x0F4DA7CF`（社区讹传）
均不是 SetPstates20——GitHub 代码搜索对 `0x0F4DA7CF` 零命中，`0x0F4DAE6B` 有
10+ 独立命中（含 firoorg/ccminer、LenovoLegionToolkit 依赖的 falahati/NvAPIWrapper、
ntminer/NtMiner、djhackersdev/bemanitools、jNizM/NVIDIA_NvAPI）。

状态码（官方 nvapi_lite_common.h）：OK=0、ERROR=-1、LIBRARY_NOT_FOUND=-2、
NO_IMPLEMENTATION=-3、API_NOT_INITIALIZED=-4、INVALID_ARGUMENT=-5、
NVIDIA_DEVICE_NOT_FOUND=-6、END_ENUMERATION=-7、INVALID_HANDLE=-8、
INCOMPATIBLE_STRUCT_VERSION=-9、HANDLE_INVALIDATED=-10、NOT_SUPPORTED=-104。
错误信息 = `NVAPI 符号名 + NvAPI_GetErrorMessage 文本`，原样上报 UI。

## 3. NV_GPU_PERF_PSTATES20_INFO 结构布局（官方 nvapi.h L1134–L1331）

三源一致：NVIDIA 官方 nvapi.h（结构声明）、falahati/NvAPIWrapper
PerformanceStates20*.cs（Pack=8，全 u32 字段）、graphitemaster/NVFC nvapi.h
（独立 C 复刻）。Rust 定义在 `hal/nvapi/ffi.rs`，布局断言测试见该文件底部。

| 结构 | 布局（偏移:字段） | 大小 |
|---|---|---|
| `ParamDelta` | 0:value(i32) 4:min(i32) 8:max(i32) | 12 |
| `ClockEntry` | 0:domainId 4:typeId 8:flags(bit0=可编辑) 12:freqDelta_kHz 24:data(联合体平铺 [u32;5]) | 44 |
| `BaseVoltageEntry` | 0:domainId 4:flags(bit0=可编辑) 8:volt_uV 12:voltDelta_uV | 24 |
| `PStateEntry` | 0:pstateId 4:flags(bit0=可编辑) 8:clocks[8] 360:baseVoltages[4] | 456 |
| `Pstates20Info` V1 | 0:version 4:flags 8:numPstates 12:numClocks 16:numBaseVoltages 20:pstates[16] | 7316 |
| `Pstates20Info` V2/V3 | V1 + 7316:numOvVoltages 7320:ovVoltages[4] | 7416 |

- 版本编码 `sizeof | (ver << 16)`（官方 MAKE_NVAPI_VERSION）：
  V1=`0x00011C94`、V2=`0x00021CF8`、V3=`0x00031CF8`（V3 结构同 V2）。
- 时钟域（官方 NV_GPU_PUBLIC_CLOCK_ID）：GRAPHICS=0（核心）、MEMORY=4（显存）、
  PROCESSOR=7、VIDEO=8。目标 pstate：P0（=0，3D Performance）。
- `data` 联合体在读取路径不解释、写入路径不携带，用 `[u32; 5]` 平铺保真透传。

## 4. 读写协议

**读取**（`NvAPI_GPU_GetPstates20`，官方公开函数）：
1. 按 V3→V2→V1 顺序探测（`NVAPI_INCOMPATIBLE_STRUCT_VERSION(-9)` 时降级，
   与 NvAPIWrapper `Accepts(V3,V2,V1)` 顺序一致）；
2. 在 `pstates[0..numPstates]` 中找 `pstateId == P0`；
3. 当前偏移 = P0 时钟表中 `domainId ∈ {GRAPHICS, MEMORY}` 条目的
   `freqDelta.value`（kHz，除以 1000 得 MHz）；驱动允许区间 = `freqDelta.min/max`。

**写入**（`NvAPI_GPU_SetPstates20`，私有接口）——最小 V1 包，
与 LenovoLegionToolkit `GPUOverclockController.ApplyPerformanceStates`（L339–360）
和 seerge/g-helper `NvidiaGpuControl.SetClocks`（L310–345）的生产实现同款：

```
version=0x00011C94, numPstates=1, pstates[0].pstateId=P0,
numClocks=n(仅请求域), clocks[i]={domainId, typeId=0(Single), flags=0,
freqDelta={value=目标kHz, min=0, max=0}}, numBaseVoltages=0
```

只携带请求域的增量：**不发送任何绝对频率、电压、其它 pstate、标志位**；
未列出的条目由驱动保持原状（两个生产工具长期验证的语义），
杜绝整表回写覆盖与“清零缓冲区重放”。

## 5. 事务与安全语义（hal/nvapi/transaction.rs + gpuctl.rs）

1. 读当前偏移（保留原始 kHz，避免 MHz 往返取整漂移）；
2. 写前校验：域存在、条目可编辑（bIsEditable）、整数 MHz、
   有效区间 = 驱动区间 ∩ 保守钳位（核心 ±300 MHz、显存 ±1000 MHz）；
   驱动报告 0..0 仅允许零偏移，倒置区间不可写；越界/只读在写入前拒绝；
3. 写最小 V1 包；4. 回读确认：请求域回读值 ≠ 请求值（精确 kHz 比较）即失败；
5. 失败（含写入报错、回读失败）→ 按原始 kHz 回滚并再次回读；
   回滚失败作为独立错误上报（硬件可能停在中间值），绝不吞掉；
6. 成功时返回回读确认的实际偏移（不是请求值）。

**基准恢复**：首次成功写入或补偿失败时记录写前原始 kHz 及 PCI 设备身份；
单项、档案和 GPU Turbo 共用首次偏移基线。独立档案快照也绑定身份；
设备替换、顺序变化或会话与枚举不一致时拒绝写入及恢复。基准一经记录不被覆盖。

## 6. 故障面（全部直报确切原因）

| 场景 | 行为 |
|---|---|
| 无 nvapi64.dll / 驱动未装 | 加载失败 + 系统错误码（如 126 ERROR_MOD_NOT_FOUND）→ `available=false` + 原因 |
| QueryInterface 返回 NULL | 指名函数与 ID 的错误（驱动过旧） |
| 无 NVIDIA 物理 GPU | 枚举失败/空 → 原因「未枚举到 NVIDIA 物理 GPU」 |
| GetPstates20 不支持（老驱动） | NVAPI 状态名 + 驱动文本 |
| 时钟只读（bIsEditable=0） | 写入前拒绝：「驱动未开放…频率偏移调节（只读时钟）」 |
| 非整数 MHz | serde 反序列化阶段拒绝（Option<i32>），前端亦拦截 |
| 越界 | 写入前拒绝，错误含交集区间 |

## 7. 当前接入

`GpuController` 统一拥有会话、互斥、设备绑定的偏移基线与 Turbo 基线；
`commands/gpu.rs`、`api/gpu.ts` 和 `components/performance/GpuTuning.tsx` 接入单项操作。
完整档案的 `gpu` 字段经 `profiles/apply/graphics.rs` 预检、捕获设备绑定的本次快照，
失败时按该快照恢复。独显关断期间释放 NVAPI 会话并阻止重连，重开后重新核对设备。
原厂事件接入 `hotkeys/` 的独立授权范围及 `api/hotkeys.ts`，不用原厂辅助程序。

## 8. 已知边界与真机验证状态

- 布局/ID/协议经官方头文件 + 两个独立生产实现交叉验证，单元测试
  （FFI 布局、写入包形状、事务/回滚/区间 mock）全部通过且不触碰 GPU/DLL；
- **未做真机写入验证**（本轮约束不触碰硬件）。首次真机验收要点：
  读取 P0 偏移与 NVIDIA 控制面板/MSI Afterburner 对照；小偏移（如 +15 MHz）
  写入后回读确认；越界值被拒；恢复基准回到原始值。
- 普通偏移与档案取驱动序首个物理 GPU；原厂 Turbo 偏移最多涉及前两张，锁频仅第一张；
  V3→V2→V1 探测在极老驱动上可能全部失败——按第 6 节故障面直报。
- `NvAPI_GPU_EnableOverclockedPstates`（0x0B23B70EE）未使用：LLT 可选调用、
  g-helper 不调用；如真机出现「写入成功但偏移不生效」再评估引入。

## 9. 原厂 GPU Turbo 事件

Pi 会话中的 FnKeyPack 7.19.5.0 使用 `ControlGPU.exe`；静态分析其托管入口和 `NVGPU_DLL.dll`：
事件96按固件温度限制锁第一张 GPU，限制为0时取实际基础图形频率；事件97按原厂模式、供电、锁频能力和已保存偏移选择目标。性能模式电池供电只清偏移；模式4无操作。完整字段见 [OEM 契约 §15](oem-dchu-contract.md)。

锁频 GET `0xE440B867`、SET `0x39442CFB` 使用780字节版本2表（`0x2030c`）：12字节头和32个24字节条目。原厂写包仅含域0/1：非零 MHz 在域0以模式2发送 `MHz*1000`，域1归零；零目标将两个域置为模式0。读取必须找到两个唯一目标域；目标及恢复均只发送这两个域，保留各域原始字段并回读，其他域不列入写包。基础时钟使用 `0xDCB616C3` 的264字节版本3结构，频率类型1和图形域有效标志必须匹配。PCI、总线与插槽身份采用 NVIDIA 公开接口并逐次核对。私有接口惰性解析，不可读、不可解析或版本错误时不写。

授权预览展示设备、当前锁频/偏移、所有模式及AC/DC目标和驱动范围错误。事件执行前对全部目标预检，逐阶段复核许可、设备、模式、供电和前态；先锁频，再按原厂顺序对最多两张卡写核心及显存偏移。失败按本次实际尝试逆序补偿；补偿失败保留基线和错误，独立恢复先停止监听并复核预览。锁表协议与设备身份由函数指针桩、内存传输及隔离页面验证；未加载 NVAPI、执行原厂程序或验收物理 GPU 效果。

## 10. 供电模式的频率事务

GPU模式的ClockObservation、ClockBaseline和ClockOrigin由`gpuctl/turbo/mode/baseline.rs`唯一拥有；供电组合记录引用这些类型，采样范围及来源一致性校验留在供电域。`powerctl/mode/links`的Context/Session/Plan统一持有节点授权与频率准备、执行、补偿及恢复；供电、手动和模式热键实际复用，原供电专属执行和仅HAL热键路径已删除。Plan在任何节点输出前复核GPU策略，驱动准备后再次核对预览原始值，检查点先于频率输出；唤醒既不读取调频策略，也不连接调频驱动。完整档案、启动与自动档案的GPU模式整合仍未完成。

`gpuctl/turbo/mode`拥有模式调校资格、单次/循环/供电六条件预检和可持久化写集；供电自动接管的通知运行入口、IPC与控制台已接入。循环一次捕获实际驱动，再投影全部可用模式的两种供电候选；未来条件不冒充当前实读。目标复用97号事件，执行复用同一GPU控制器、首个偏移基线和失败补偿；模式调校不读取96号事件才需要的基础频率。

在线预览包含当前原始锁频/偏移和驱动编辑范围，分别校验接电、断电、交流/电池启动及交流/电池唤醒。条件目标与当前实际供电读数分开，不能把假定的未来供电当成实读；当前独显关闭时只声明延后核对，预览不因此重连。设备/驱动能力绑定稳定身份，原始读数绑定预览修订，范围失败保留具体候选原因。前端公共GPU类型由`src/api/gpu.ts`唯一拥有，热键及供电复用。

`RawState`只保存有序PCI/总线/插槽身份、第一张卡的两个所属锁项，以及前两张卡的原始kHz偏移。锁项包含原始未知字段；不序列化驱动句柄，不经MHz换算。写集同时保存实读前态和预计目标，核对原厂报告的显卡数量及非零缓存扩展设备编号；缓存编号0不表示没有显卡。范围、可编辑标志、设备或前态变化时拒绝旧目标，持久回调失败时零GPU输出。

供电组合记录区分当前频率实读与首次频率恢复基线，后者保留`beforeMode / afterEnable / poweredOff`来源。独显控制节点关闭时不伪造当前读数；若按已确认事务取得了首次可读前态，必须在GPU写入前持久化。跨域后续失败先恢复已尝试字段，显式恢复逐字段核对当前预览，发送原始值并回读；设备变化时拒绝向新卡恢复。

普通监控和单项调校在独显关断时继续禁止重连。原厂模式事务可能在节点关闭时调用GPU助手，因此已确认的模式/恢复入口可持有短时控制会话；许可先于连接，成功、错误及异常展开均释放临时连接，保持原暂停状态，不提前开启节点或改变自动关断偏好。隔离实例仍必须注入提供者，缺失时禁止转向真实驱动。此入口不供页面轮询使用。
