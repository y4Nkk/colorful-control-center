# Agent Note: 概览频率读数与卡片内温度折线

Status: implemented

## 问题

- 概览 CPU/GPU 卡片的大读数是风扇转速 RPM，与性能与散热页的风扇实时读数重复；概览改为频率导向，CPU 显示当前频率、GPU 显示当前核心频率。
- 硬件快照没有 CPU 当前频率字段；CPU 规格弹窗的「标称最高频率」依赖 CPUID leaf 0x16 EBX，本机该字段为空显示「未知」，不能作为数据源。
- 温度趋势是独立面板（热度时间带），改为取消独立模块，把 CPU/GPU 温度以折线嵌入各自卡片。

## 决定

2026-11-20 概览卡片大读数改为「当前频率」，单位 MHz：

- CPU 频率取 Windows PDH `\Processor Information(_Total)\% Processor Performance` 乘以**同一计数器组**的 `\Processor Information(_Total)\Processor Frequency`（与任务管理器「速度」同源语义），并入现有 `cpu_utility` 采样与节流；`cpuUtility` 读取增加 `clockMhz`。基频由 PDH 计数器拥有：`_Total` 的 `Processor Frequency` 是按逻辑处理器加权的标称基频，与 `% Processor Performance` 的分母严格配套；频率计数器缺失或无效时 `clockMhz` 为空，不回退 CPUID。
- GPU 直接使用已有 NVAPI `graphicsClockMhz`。
- 风扇转速归性能与散热页。概览卡片把频率、利用率、风扇占空比和 GPU 显存排入同一指标网格，每项由无底色的单色线条图标、标签和实读数值组成。宽窗指标与图表内容共用横向内边距。移除比例横条，频率直读 MHz。显存指标显示已用量，总量可从 GPU 规格或读屏读数获取。
- CPU/GPU 卡片以最近 5 分钟的温度面积图为主，显示当前温度、窗口峰值、相对时间刻度及共享的 20–110 °C 绝对标尺。95 °C 及以上有阈值底色，高温读数和标记用红色；断线与面积共用 `timeTrace` 计算，缺测不补值。`TemperatureHistory` 时间带组件与样式已移除。
- 概览移除常驻的采样时间与信息图标。规格使用文字入口。状态行显示连接、读数停滞、风扇通道离线与独显监控暂停，并提供硬件采样重试。内存图表沿用五分钟范围文案与采样点悬停提示。
- 2026-11-23 移除「监控诊断」入口及弹窗。系统资料依靠现有轮询自动恢复，失败时清空资料并在内存趋势中保留缺口。模拟界面回归覆盖收到采样与系统告警时入口仍不出现。

### 契约

- `cpu_utility::Reading` 增加 `clock_mhz: Option<f64>`；效用与频率同一次 PDH 采样产生、共用时间戳；频率计数无效时不回退旧值、不影响效用值，`clockMhz` 为空。
- 前端 `HardwareSnapshot.cpuUtility.clockMhz`；`HardwareReadings` 拥有卡片读数与内嵌温度折线（组件 `SensorTemperature`），`ControlWorkspace` 不再渲染温度趋势面板。
- 频率读数与温度折线不提供分母或占比语义；温度折线是绝对标尺的位置图，不重复冒充实测转速。

## 影响与验证

2026-11-20 补记：`npm run build`（tsc + 边界检查 + vite build）与 `cargo check` 在包含本改动的工作树上通过。本节由后续弹窗层级改动会话补写；该会话未运行概览频率与温度折线的专项模拟 IPC 检查，也未做真机读数核对。

2026-11-20 真机修复：本机（i5-13500HX，Raptor Lake-HX）CPUID leaf 0x16 存在但 EAX/EBX/ECX 全为 0，首版「CPUID 基频 × 性能百分比」实现因此频率恒为空（概览显示 `—`）。真机探针确认 PDH `\Processor Information(_Total)\Processor Frequency` 返回 2220 MHz——即 P 核 2500 / E 核 1800 按 20 个逻辑处理器加权——且 `% Processor Performance` 同组读数正常，故基频来源改为该计数器。

## 风险

PDH `Processor Frequency` 计数器与性能计数器位于同一计数器组，若系统未提供该组（虚拟机等）则效用与频率一并降级为空；频率计数器单独缺失或状态无效时 `clockMhz` 为空，不回退 CPUID。PDH 效用计数器与任务管理器的采样窗口不同，读数可能与任务管理器短暂不一致。温度折线固定 20–110 °C 标尺，低于 20 °C 的环境读数会贴底显示，不代表精确数值。移除 `TemperatureHistory` 后，热度时间带的既有读者失去长时间跨度视图，仅剩 5 分钟窗口。

### 2026-11-23 审查修正

导航和概览回归使用各自 CPU/GPU 卡的运行读数、MHz 和内嵌折线契约。将旧时间带断言替换为真实时间位置、长间隔与缺测断线、高温标记、当前空值和窄屏溢出检查，恢复用例后续的档案切页、键盘操作及布局断言。

## 备选方案

- 拒绝沿用 CPU 规格弹窗的「标称最高频率」（CPUID leaf 0x16 EBX）作为概览频率数据源：本机该字段为空，且是规格值而非当前读数。
- 拒绝在概览保留风扇 RPM 大读数：与性能与散热页实时读数重复，信息冗余。
- 拒绝保留独立温度趋势时间带面板：单卡片内嵌折线已覆盖「看趋势」诉求，独立模块增加一页一卡维护成本。
