# Agent Note: 硬件参数列表与中文调节说明

Status: implemented

## 问题

用户要求 CPU、GPU 等模块同时列出硬件参数和解释调节项，随后明确 PL1、PL2、GT 等参数名在界面统一为中文。现有模块以插画和少量比例读数为主，缺少规格列表与术语解释；参数缩写在调节、预览、档案及后端消息中不一致。

## 决定

- hal/overview.rs 继续拥有系统只读概览；CPU 子模块提供 Windows 活动物理核心/逻辑处理器数量及 CPUID 声明的基础/最高频率，缺少声明时为未知，不冒充实时频率。固定磁盘卷补充卷标与文件系统，不把逻辑卷当物理硬盘型号。
- hal/nvidia.rs 在既有只读采样中补充核心/显存频率、功耗/当前功耗上限、计算核心数量和显存位宽；不支持的单项为 null，保留其余读数。Rust、TypeScript 与隔离夹具同次更新，不改硬件写入契约。
- CPU/GPU/内存/存储卡片使用 FactList 列出真实参数；领域 ParameterHelp 复用全局 Collapsible，解释规格、功耗/电压、频率偏移及两类风扇曲线。离线时仍可读说明，不请求额外硬件写入。
- 持续功耗上限、短时功耗上限、核心电压偏移、核显电压偏移、核心频率偏移、显存频率偏移在控件、差异预览、档案、自启预览和用户可见后端消息中同名；界面及联动预览统一使用 Turbo 风扇、动态调校 DTT、链路节能、能效优先和寂静模式。CPU/GPU 部件简称、标准单位和内部协议/JSON 标识保留，不重命名稳定 IPC 字段。
- 参数表与解释仅使用既有表面、折叠、事实行和设计令牌，不增加依赖，不补写虚构的内存类型/速率或磁盘型号。
- 原厂 CPUOC 的 GetRAMClock 实际读取 page6[23..27] 小端有符号频率，IndexViewModel 从 page6[230..232] 读取无符号参考频率。本次扩展由 hal/memory.rs 唯一解码，并入当前 HardwareSnapshot；只有本次 cmd12 传输成功才读取共享报告。两值独立失败或未报告时为未知，不将共享报告当作直接实测，不转换为 DDR 有效速率，也不以参考值限制或绘制频率比例。监控采样与原有风扇解码迁入 hal/monitor.rs，移除根模块旧实现。

## 影响与验证

2026-11-15 内存频率扩展：13项HAL聚焦检查（5项频率解码/序列化、4项既有风扇解码、4项采样连接）、6项软件风扇回归及12项真实隔离软件风扇进程检查通过；覆盖精确页范围、正负和零值、独立失败、GPU暂停不影响内存、断连后旧值消失及恢复。8项模拟页面检查通过，含5项内存和3项控制台回归；覆盖实时更新、超过参考值、未知/离线、只读查看、说明和360/700/1440px深浅布局。已目检浅色宽卡和深色窄卡，截图在target/ui-check/memory-monitor-*.png。npm构建、主程序与control-sim离线编译、源码500/80和笔记检查通过；仅有既有构建体积及增量缓存复制警告。没有加载真实DLL或访问硬件；共享报告新鲜度及物理频率仍未真机验收。

验证覆盖 CPU 拓扑记录边界/缺失频率、序列化及 UI 的已知/未知参数、中文名称、说明展开、跨页保活和响应式布局；复用既有隔离模式操作验证，不启动真实控制器。

原生 4 项 CPU/序列化聚焦测试通过；独立只读程序直接复用生产 overview 模块，读到本机 i5-13500HX 的 14 个活动核心、20 个逻辑处理器和固定卷的卷标/文件系统。该机 CPUID 未提供两项标称频率，保留 null，不按型号猜测。GPU 新增查询尚未做真机读取验收，页面以模拟快照验证。未执行真实硬件写入、软件偏好保存或完整测试套件。

最终 npm run build（含 TypeScript、全局边界检查、Vite）、cargo check --offline --workspace 与笔记校验通过；即时处理器域 5 项聚焦回归通过，加上上述 4 项原生检查共 9 项。实际构建页面以隔离 IPC 完成 76 项检查，覆盖原有模式链路、参数列表/说明、中文可访问名称、档案与启动摘要、未知数据、无写入查看、浅深主题及 390px 布局；无未处理页面错误。截图与完整结果在 target/preset-modes-ui/，只读探针源码在 target/parameter-read-probe/。

中文化检索覆盖 src 与用户可见 Rust 消息；删除独立 FAN_NAMES 重复表，统一复用 FAN_MODES。保留内部 API 字段、ASPM_NAMES 变量及模拟夹具中的协议事件标签，不把内部标识和测试轨迹当界面文案修改。

术语与读取约定核对 [Intel 功耗控制文档](https://edc.intel.com/content/www/de/de/design/products/platforms/details/raptor-lake-s/13th-generation-core-processors-datasheet-volume-1-of-2/002/007/005/package-power-control/)、[Microsoft CPU 拓扑 API](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getlogicalprocessorinformationex) 及 [NVIDIA NVML 查询文档](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html)；Windows 与 NVML 调用签名同时对照当前已安装依赖源码。

## 风险

本次只扩展只读参数及说明；当前功耗上限不是显卡出厂最大功耗，CPUID 标称频率不是实时频率。驱动可能不支持部分 NVML 查询，单项必须保留未知。硬件写入及稳定性未做真机验收。

## 备选方案

- 按型号查询表填充规格：会将未经读取的营销规格冒充本机状态，拒绝。
- 只改主控件文字：会留下档案、预览和错误提示中的旧名称，拒绝。
- 新建提示浮层或说明页面：已有 Collapsible 与 FactList 足够，保留模块内上下文。
