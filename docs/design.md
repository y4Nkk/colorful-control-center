# Colorful Control Center 设计文档

> 开发基准机：七彩虹将星 X15 AT 23（i5-13500HX + RTX 4060 Laptop GPU，蓝天/联宝模具）
> UI 参考：`agent-switchboard` 项目（安静清晰的本地工具风）

## 1. 目标与痛点

OEM Control Center 可调参数不满足需求。本项目的目标是补齐：

- **真正的风扇曲线**：原版只有 自动/最大/自定义（固定档位），本项目提供任意温度-转速曲线
- **功耗墙调节**：PL1/PL2 独立可调（原版只有四档预设）
- **偏移电压（undervolt）**：优先级高，Phase 0 必须验证 MSR 0x150 是否被固件锁定
- **GPU 调校**：核心/显存频率偏移
- **性能档案**：上述参数打包一键切换

## 2. 硬件控制通道（按优先级）

> 2026-11-12 Phase 0 修订：基准机逆向审查（docs/oem-dchu-contract.md）确认
> root\WMI 无 OEM 类；OEM CC 实际通道是 **InsydeDCHU.dll + AcpiBridge.sys 驱动**
> （已安装、签名、覆盖读+写+持久化）。架构据此调整：

| 层 | 通道 | 用途 | 依赖 |
|---|---|---|---|
| T1 | OEM InsydeDCHU.dll（FFI） | 温度/频率/风扇 RPM+duty/曲线读写/模式/PL+降压持久化 —— OEM CC 全部功能走这里 | AcpiBridge.sys（已装）；随 OEM CC 卸载而失效 |
| T2 | EC 端口直写（62h/66h） | 兜底：风扇 duty（0x2C/0xCF/0xE7，tools/probe/ec-map.json） | PawnIO 或 WinRing0（仅当 T1 不可用且用户显式启用）|
| T3 | CPU MSR | PL1/PL2（0x610）、偏移电压（0x150）运行时直写、锁检测 | PawnIO（msr-driver-rs）或 WinRing0 |
| T4 | NVML + NVAPI | GPU 传感器读数（NVML）、频率偏移（NVAPI SetPstates20） | 用户态 DLL |

分层规则：同一功能先走 T1，失败或粒度不足再降级 T2/T3。策略引擎只面向 HAL 接口，不感知具体通道。

已知参考（详见 docs/oem-dchu-contract.md）：

- T1 完整命令契约已从 OEM CC 反编译提取：读 cmd 12/13/18/29/85/96/101，
  写曲线 cmd 14、风扇模式 cmd 121 sub1/14/25/34、PL+降压持久化 AppSettings page6
- `NvAPI_GPU_SetCoolerLevels` 在 RTX 上不支持（GPU 风扇归 EC/DCHU 管）——T1 通道已覆盖
- 笔记本 GPU 功耗目标常被 vBIOS 锁定，NVAPI 设置可能失败；频率偏移一般可用
- 电源模式固件写入命令未提取到（OEM 走 native Fn 守护进程），启用前需真机验证

## 3. 软件架构

```
┌──────────── UI 层（React + TS，工具风）────────────┐
│  监控仪表盘 │ 风扇曲线编辑器 │ 性能档案 │ 设置/托盘  │
├──────────── 策略引擎层（Rust）─────────────────────┤
│  风扇曲线引擎（插值+滞后+斜坡限制）                 │
│  性能档案（PL/EPP/GPU偏移/曲线 打包）               │
│  安全看门狗（崩溃恢复、温度熔断）                   │
├──────────── 硬件抽象层 HAL ─────────────────────────┤
│  OemWmiProvider   → T1                             │
│  EcProvider       → T2（PawnIO/WinRing0 后端抽象） │
│  IntelCpuProvider → T3                             │
│  NvidiaGpuProvider→ T4                             │
└─────────────────────────────────────────────────────┘
```

### 内核访问后端

2026-11-12 修订：T1（InsydeDCHU）覆盖了原计划中需要内核驱动的绝大多数能力
（EC 读写、风扇曲线、PL/降压持久化），因此常规路径**不需要自带内核驱动**，
也规避了 WinRing0 的 Defender VulnerableDriver 拦截。

仍需内核访问的场景（T3 运行时 MSR 直写 / T2 EC 兜底）由 `KernelAccess` trait
抽象，两个实现：

1. **PawnIO**（首选）：签名驱动，不触发 Microsoft Defender 的 VulnerableDriver 拦截（WinRing0 被 CVE-2020-14979 标记）；msr-driver-rs 封装了 MSR 读写与安装
2. **WinRing0**（后备）：PawnIO 不可用时的兼容路径（参考 winRing0-rs 的实现）

### 与 OEM Control Center 共存

检测到 OEM CC 服务运行时提示用户退出，避免双方争抢 EC。

## 4. 安全设计

1. **崩溃恢复**：EC 写入手动模式后软件若死亡，风扇可能停在最后一次占空比。双进程看门狗，检测到策略进程崩溃立即恢复风扇自动模式
2. **温度熔断**：任一传感器 > 95°C 强制恢复自动模式
3. **写入钳位**：占空比下限 ≥ 20%（防失速）；PL 上限按基准机散热能力（起步 80W 观察）；永不写入 HAL 范围外的值
4. **只读先行**：Phase 1 只做监控，读数与 HWiNFO 对齐后才开启写入

## 5. Phase 0 探测清单

> 2026-11-12 完成（方式：审查机器上 OEM CC 实现 + nbfc-linux 社区配置，无需交互式真机调试）：

1. **识别 ODM** ✅ — `tools/probe/wmi-enum.ps1` + `tools/probe/wmi-enum-results.txt`：
   root\WMI 无 OEM 类；通道确认为 InsydeDCHU.dll（x64）+ AcpiBridge.sys（Running）
2. **NBFC 现成配置** ✅ — `Colorful X15 AT 22.json`（同模具上一代）：
   手动控制权 0x2C=8/auto=5、CPU 风扇写 0xE7 / 读 0xCF（20-100）
3. **EC/接口映射** ✅ — 升级为 DCHU 契约：`docs/oem-dchu-contract.md`
   （读 cmd12/13、曲线 cmd14、模式 cmd121、PL+降压 page6、能力位图）+
   `tools/probe/ec-map.json`（原始 EC 兜底，未验证）
4. **MSR 锁状态** ⬜ — probe-cli 待实现（0x610 bit63 / 0x150 写入测试；
   本机 XTU3SERVICE 已安装但 Stopped，page6+XTU 为降级路径）

## 6. UI 方向

UI / 交互契约由根目录 [DESIGN.md](../DESIGN.md) 唯一拥有（视觉语言、令牌所有权、组件所有权、页面结构、禁止项）。本文保留硬件控制通道架构、安全设计与 Phase 0 清单的事实所有权。

## 7. 参考项目

- [nbfc](https://github.com/hirschmann/nbfc) / [nbfc-linux](https://github.com/nbfc-linux/nbfc-linux) —— EC 风扇控制逻辑与机型配置库
- [tuxedo-drivers](https://github.com/tuxedocomputers/tuxedo-drivers) —— Uniwill/蓝天 WMI 接口的权威实现
- [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) —— MSR/传感器读数逻辑参考
- [winRing0-rs](https://github.com/alex-dow/winRing0-rs) / [msr-driver-rs](https://crates.io/crates/msr-driver-rs) / [PawnIO](https://github.com/namazso/PawnIO) —— 内核访问
- [NvAPIWrapper](https://github.com/falahati/NvAPIWrapper) / [nvml-wrapper](https://github.com/Cldfire/nvml-wrapper) —— GPU 侧
