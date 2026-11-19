import type { ParameterDescription } from "./ParameterHelp";
import { PERFORMANCE_OPTION_LABELS } from "../../api/client";

export const CPU_SPEC_HELP: readonly ParameterDescription[] = [
  { name: "物理核心与逻辑处理器", description: "分别表示系统当前启用的实际处理核心数量和可调度的硬件线程数量。" },
  { name: "标称基础频率与标称最高频率", description: "处理器报告的频率规格，以 GHz 表示。实际运行频率会随负载、温度和功耗变化。" },
  { name: "温度与风扇读数", description: "温度以 °C 表示；风扇转速以 RPM 表示每分钟转数，占空比以 % 表示控制比例。未取得的读数显示未知。" },
  { name: "CPU 利用率", description: "采用原厂使用的 Windows 处理器效用计数器，显示最近采样结果；按原厂规则最高显示 100%，启动或采样失败时显示未知。" },
];
export const GPU_SPEC_HELP: readonly ParameterDescription[] = [
  { name: "计算核心数与显存位宽", description: "显卡报告的并行计算核心数量与显存数据通路宽度。位宽以位表示，不能单独代表游戏性能。" },
  { name: "当前核心频率与当前显存频率", description: "当前采样的运行频率，以 MHz 表示，会随工作负载变化。调节区中的频率偏移是在默认策略上叠加的调整量。" },
  { name: "显存总量与显存占用", description: "分别表示显卡报告的容量和已用量，以 MB、GB 等单位显示。显存与系统内存分别统计。" },
  { name: "当前功耗与当前功耗上限", description: "显卡报告的耗电量与当前配置的功耗限制，以 W 表示。当前上限不代表出厂最大功耗或保证能够持续达到的功耗。" },
];
export const MEMORY_HELP: readonly ParameterDescription[] = [
  { name: "总容量、已用与可用", description: "展示系统报告的物理内存容量、已用量和可用量，后台程序与系统缓存会影响读数。" },
  { name: "容量单位", description: "以 Windows 口径显示容量。1 KB 为 1024 字节，1 MB 为 1024 KB，1 GB 为 1024 MB。" },
  { name: "原厂报告频率与原厂参考频率", description: "沿用原厂共享报告中的 MHz 数值，可能有更新延迟，未报告时显示未知。参考频率不是硬上限，不据此计算占比；这两项不表示直接实测的时钟或换算后的 DDR 有效传输速率。" },
];
export const STORAGE_HELP: readonly ParameterDescription[] = [
  { name: "盘符、卷标与文件系统", description: "盘符标识系统中的磁盘卷，卷标是用户设置的名称，文件系统表示数据组织格式。一个物理硬盘可以包含多个磁盘卷。" },
  { name: "总容量、已用与可用", description: "按当前账户可访问的固定磁盘卷统计。可用容量可能受账户配额影响。" },
];
export const CPU_TUNING_HELP: readonly ParameterDescription[] = [
  { name: "持续功耗上限 · W", description: "限制处理器长时间运行的平均功耗。提高上限会增加可用功耗预算，也可能增加发热与散热需求。" },
  { name: "短时功耗上限 · W", description: "限制短时加速阶段的功耗预算；不能低于持续功耗上限。它是限制值，实际功耗还受温度和负载影响。" },
  { name: "核心电压偏移 · mV", description: "在处理器核心电压策略上叠加的调整量。负值降低电压偏置，过低可能造成程序出错、死机或重启。" },
  { name: "核显电压偏移 · mV", description: "作用于处理器内置图形核心的电压偏置；0 表示不额外偏移。可调范围以当前控件显示为准。" },
  { name: "立即应用与保存设置", description: "立即应用请求调整当前运行参数；保存设置写入原厂保存值，保存成功不等于当前运行参数已改变。应用后以回读结果为准。" },
];
export const GPU_TUNING_HELP: readonly ParameterDescription[] = [
  { name: "核心频率偏移 · MHz", description: "在显卡默认核心频率策略上叠加的调整量；0 表示不额外偏移，实际频率仍随负载与温度变化。" },
  { name: "显存频率偏移 · MHz", description: "调整显存运行频率的偏移量。可调范围由当前显卡能力决定；过大的偏移可能造成花屏或驱动重置。" },
  { name: "恢复初始", description: "恢复本次应用会话首次修改前记录的偏移值。控件为只读或未知时不能写入。" },
];
export const FAN_HELP: readonly ParameterDescription[] = [
  { name: "温度阈值 · °C", description: "曲线横轴表示温度，纵轴表示散热目标。固件两点曲线的占空比与十级曲线的目标转速百分比分别按当前机型能力显示。" },
  { name: "占空比与目标转速 · %", description: "占空比是风扇控制比例；目标转速百分比按该风扇最大转速换算。两者都不保证与实际 RPM 成线性关系。" },
  { name: "加速偏移 · %", description: "原厂风扇策略的额外加速偏移，与曲线目标分别预览，确认后才应用。" },
];
export const SOFTWARE_FAN_HELP: readonly ParameterDescription[] = [
  { name: "控制温度与占空比", description: "按 CPU 与 GPU 中较高温度调节风扇，曲线点之间插值。软件曲线在应用运行期间生效。" },
  { name: "回差 · °C", description: "降温时保留一定温度余量再降低风扇目标，减少温度小幅波动造成的频繁调速。" },
  { name: "斜坡 · %/秒", description: "限制每秒占空比变化的幅度，用于控制调速快慢。安全恢复条件优先于普通曲线调节。" },
];
export const MODE_HELP: readonly ParameterDescription[] = [
  { name: "原厂模式与风扇策略", description: "模式会联动当前机型保存的风扇及电源策略。每次应用以预览中的具体目标为准。" },
  { name: `${PERFORMANCE_OPTION_LABELS.turboFan}与${PERFORMANCE_OPTION_LABELS.dtt}`, description: "Turbo 风扇启用原厂风扇加速。动态调校 DTT 通过驱动调节设备功耗和散热。选项随已预览的模式应用。" },
  { name: "链路节能", description: "控制设备数据链路的节能级别，分别预览插电与电池供电时的目标。" },
  { name: "电源计划", description: "切换系统的平衡、高性能或节能计划。模式名称描述使用倾向，具体功耗仍以当前限制和读数为准。" },
];
