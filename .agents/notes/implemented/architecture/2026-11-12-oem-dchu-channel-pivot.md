# Agent Note: 硬件主通道转向 InsydeDCHU（放弃自带内核驱动作为常规路径）

Status: implemented

## 问题

原架构（docs/design.md v1）规划 T1=OEM WMI、T2=EC 直写（PawnIO/WinRing0）、T3=MSR，其中常规能力（风扇、EC 读写）依赖自带内核驱动，带来 Defender VulnerableDriver 拦截风险与分发负担。Phase 0 探测需要真机交互（EC diff 逐项切设置），用户要求避免真机调试，授权改为审查机器上 OEM Control Center 的实现并参考高星开源项目。

## 决定

（2026-11-12，基于基准机逆向审查事实）

1. **主通道 = InsydeDCHU.dll FFI（T1）**：OEM CC 的真实通道。root\WMI 实测无任何 OEM 类；OEM 全部能力（温度/频率/风扇 RPM+duty/自定义曲线/风扇模式/PL/降压持久化/能力位图）通过 InsydeDCHU.dll（x64）+ 已安装的 AcpiBridge.sys 驱动实现。完整命令契约提取至 docs/oem-dchu-contract.md。
2. **Phase 0.3 交付物升级**：不再做交互式 EC diff 探测；接口映射以反编译契约（oem-dchu-contract.md）+ nbfc-linux 社区配置（tools/probe/ec-map.json，兜底原始 EC 寄存器 0x2C/0xCF/0xE7）交付。
3. **内核驱动只保留给 T3 运行时 MSR**（PL/undervolt 直写，probe-cli 先测锁）与 T2 兜底，首选 PawnIO；WinRing0 仅作后备且默认不启用。
4. 降级链明确：T1 不可用（OEM CC 被卸载）→ HAL 报告只读不可用 → 用户显式启用才走 T2 EC 兜底。

## 影响

- 常规安装不再分发任何内核驱动，规避 Defender 拦截与签名负担。
- 依赖 OEM 组件存在（AcpiBridge.sys / CCDCHUService / InsydeDCHU.dll 路径），HAL 启动时探测并报告；OEM CC 升级可能改变 DLL 路径，需容错。
- 风扇曲线、模式、PL 持久化与 OEM CC/固件语义完全一致，写入风险大幅降低（走 OEM 同款协议而非裸 EC）。
- 电源模式固件写入命令仍未知（OEM 在 native Fn 守护进程里），标记为待验证项，禁止盲写。
- docs/design.md §2/§5/内核访问节已同步修订；tools/probe/wmi-enum.ps1 + 结果存档为 Phase 0.1 证据。

## 备选方案

- **原方案（OEM WMI 为主）**：被实测否定——root\WMI 无 OEM 类，该机器根本没有 WMI 接口面。
- **裸 EC 直写为主（NBFC 式）**：可行（寄存器已有社区配置）但语义粗糙（只有 duty 0-100，无曲线/模式/PL），且必须自带内核驱动、承担失速风险；降级为兜底而非主通道。
- **继续交互式 EC diff 探测**：信息收益已被反编译覆盖，且需要用户配合切 OEM 设置，违背"避免调试机器"约束。放弃。
