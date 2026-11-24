import type { ParameterDescription } from "./ParameterHelp";
import { PERFORMANCE_OPTION_LABELS } from "../../api/client";

export const CPU_SPEC_HELP: readonly ParameterDescription[] = [
  { name: "物理核心与逻辑处理器", description: "物理核心是 CPU 实际的核心数量；逻辑处理器是系统能同时安排任务的线程数。两项都是系统当前启用的数量。" },
  { name: "标称基础频率与标称最高频率", description: "厂商标称的基础频率和最高频率，单位 GHz。实际运行频率会随任务负载、温度和功耗变化，不一定等于这两个值。" },
  { name: "温度与风扇读数", description: "温度单位是 °C。风扇转速单位是 RPM，即每分钟转数；占空比是送给风扇的控制比例，单位 %，和实际转速不一定成正比。读不到的数据显示「未知」。" },
  { name: "CPU 利用率", description: "CPU 当前的忙碌程度，单位 %。数据来自 Windows 的「处理器效用」计数器，和电脑自带控制中心用的是同一个来源；最高显示 100%，启动或读取失败时显示未知。" },
];
export const GPU_SPEC_HELP: readonly ParameterDescription[] = [
  { name: "计算核心数与显存位宽", description: "计算核心数是显卡里并行计算核心的数量；显存位宽是显存传输数据的通道宽度，单位为位。位宽不能单独用来判断游戏性能。" },
  { name: "当前核心频率与当前显存频率", description: "刚读取到的 GPU 核心和显存运行频率，单位 MHz，会随任务负载变化。在「性能与散热 → GPU 频率」里调的偏移，是在默认频率基础上加减的量。" },
  { name: "显存总量与显存占用", description: "显存是显卡自带的内存。总量是它的容量，占用是已经用掉的部分，单位 MB、GB 等。显存和系统内存分开统计。" },
  { name: "当前功耗与当前功耗上限", description: "当前功耗是显卡此刻的耗电量，功耗上限是当前设置允许的最大功耗，单位 W（瓦）。功耗上限不等于出厂最大功耗，也不保证能一直达到。" },
];
export const MEMORY_HELP: readonly ParameterDescription[] = [
  { name: "总容量、已用与可用", description: "总容量是装机的内存总量，已用和可用是系统报告的当前用量和剩余量。后台程序和系统缓存都会占用内存，所以读数会一直变化。" },
  { name: "容量单位", description: "容量按 Windows 的算法显示：1 KB 是 1024 字节，1 MB 是 1024 KB，1 GB 是 1024 MB。" },
  { name: "内存频率", description: "内存条配置的 DDR 有效传输速率，单位 MT/s（每秒百万次传输）。数值来自主板记录。内存条报告的速率不一致或缺少数据时，显示「未知」。" },
  { name: "原厂报告频率与原厂参考频率", description: "这两项来自电脑自带控制中心，单位 MHz，更新可能有延迟。参考频率只供对照。要看内存条配置的传输速率，请看上方以 MT/s 显示的「内存频率」。" },
];
export const STORAGE_HELP: readonly ParameterDescription[] = [
  { name: "盘符、卷标与文件系统", description: "盘符是 C:、D: 这样的字母标识；卷标是给这个分区起的名字；文件系统是这块分区存放数据的方式。一块硬盘可以分成多个分区，各有自己的盘符。" },
  { name: "总容量、已用与可用", description: "只统计当前账户能看到的本地磁盘分区。如果账户设置了磁盘配额，「可用」可能比磁盘实际的剩余空间小。" },
];
export const CPU_TUNING_HELP: readonly ParameterDescription[] = [
  { name: "持续功耗上限 · W", description: "限制 CPU 长时间运行时的平均功耗，单位 W（瓦）。调高上限后，高负载时 CPU 可以用更多电，也可能产生更多热量。" },
  { name: "短时功耗上限 · W", description: "限制 CPU 短时间爆发（比如刚打开大型程序时）的功耗，单位 W。不能设得比持续功耗上限低。它只是上限，实际功耗还取决于温度和任务负载。" },
  { name: "核心电压偏移 · mV", description: "在 CPU 核心默认电压的基础上加减的量，单位 mV（毫伏）。负值表示降低电压，能减少发热，但降得过多可能导致程序出错、死机或自动重启。" },
  { name: "核显电压偏移 · mV", description: "和核心电压偏移一样，只是作用在 CPU 内置的核显上。0 表示不调整。能调的范围以滑杆显示的为准。" },
  { name: "立即应用与保存设置", description: "「立即应用」只调整 CPU 当前的运行参数；「保存设置」把数值写入电脑的保存项，但不代表当前运行参数已经改变。两种方式应用后，都以重新读取到的结果为准。" },
];
export const GPU_TUNING_HELP: readonly ParameterDescription[] = [
  { name: "核心频率偏移 · MHz", description: "在显卡默认核心频率的基础上加减的量，单位 MHz。0 表示使用默认设置。调高后可能提升性能，也可能增加发热。实际频率仍会随负载和温度变化。" },
  { name: "显存频率偏移 · MHz", description: "在显存默认频率的基础上加减的量，单位 MHz。能调的范围由显卡决定。调得过高可能出现花屏，或者被驱动自动重置。" },
  { name: "恢复初始", description: "把核心和显存偏移恢复成这次打开软件后、第一次修改之前的数值。显卡当前不允许调节，或偏移读数未知时，无法恢复。" },
];
export const FAN_HELP: readonly ParameterDescription[] = [
  { name: "温度阈值 · °C", description: "风扇曲线的横轴是温度（°C），纵轴是想让风扇达到的转速。不同机型能调的方式不同：有的只能设两个温度点和对应的占空比，有的可以分十档温度分别设定目标转速。" },
  { name: "占空比与目标转速 · %", description: "占空比是送给风扇的控制信号比例；目标转速是相对该风扇最大转速的百分比。两者都是目标值，和实际转速（RPM）不一定成正比。" },
  { name: "加速偏移 · %", description: "在电脑自带风扇方案的基础上额外加速风扇的幅度。调好后松开滑杆或结束键盘调整就会生效。" },
];
export const MODE_HELP: readonly ParameterDescription[] = [
  { name: "预设模式与风扇、电源", description: "每个预设模式都会一起切换这台电脑保存的风扇和电源设置。具体会改动哪些设置，以预览里列出的内容为准。" },
  { name: `${PERFORMANCE_OPTION_LABELS.turboFan}与${PERFORMANCE_OPTION_LABELS.dtt}`, description: "Turbo 风扇提高转速来加强散热，运行时可能更吵。动态调校 DTT 交给 Intel Dynamic Tuning 驱动自动调节功耗和散热。两项在「性能选项」里单独设置，切换预设模式时会按保存的开关状态一起应用。" },
  { name: "链路节能", description: "控制电脑内部设备连接通道（PCIe）的省电程度。省电越多，设备从省电状态恢复时可能略有延迟；插电和用电池时的设置分别预览。" },
  { name: "电源计划", description: "切换 Windows 的电源计划：平衡、高性能或节能。预设模式的名字只表示使用倾向，实际功耗以当前功耗上限和读数为准。" },
];
