# Agent Note: 任意点软件风扇曲线的原厂 ACPI 通道

Status: implemented

## 问题

此前软件闭环缺少独立 CPU/GPU duty 写入事实，AT22 的 CPU 0xE7 不能证明 GPU 0xE8，两点/十级固件曲线也不等于软件闭环。同模具项目提供 PK04/C1 线索后，必须以本机 FnKey、DLL 与 DSDT 独立确认，不能照搬其他版本。

## 决定

只读导出本机 551584B DSDT（SHA256 d5162a4a002576ad9dcc77115001fbc66801927d7890c1316b9839a3c14b5203），官方 iASL 20260408 反编译。CC30/PK04 转发到 ECMD，SCMD 0x69 按掩码发送 C1(0xFF, channel)，手动则是 C1(channel, duty)，100% 不等同于自动。事实归属 docs/oem-dchu-contract.md §8；没有执行这些方法。

- 本次仅审查原厂反编译代码、DLL 和固件方法体，核对 PK04 转发、通道选择、返回值及恢复自动；不执行 ACPI 方法、EC/MSR 命令或真实硬件测试。
- DSDT 用 Windows GetSystemFirmwareTable 只读复制到本会话隔离目录，再用官方 ACPICA iASL 反编译；不安装/加载表，不写注册表、不更改驱动/服务。
- ccc-core 拥有逐通道包、换算与自由曲线契约；fanctl/software.rs 拥有定时执行、模式互斥及会话，HAL 保存状态。控制台、档案和恢复均接通；删除未使用的“自由曲线压缩成两点”函数及测试。
- 软件模式只在 DSDT 哈希及 CPU/GPU1 存在位匹配时开放；新增 sha2 进行识别，Windows API 仅复制字节。监护子进程必须独立确认恢复能力才 READY/ARM。每把 2–12 个整数点，温度 20–95°C 严格递增、占空比 20–100% 不降、末点 95°C/100%；整数回差 0–10°C、斜坡 1–100 百分点/秒。使用 CPU/GPU 较高温度，GPU 通道初始缺传感时拒绝，运行中曾出现的 GPU 丢失时恢复。
- 500ms 独立调度，进入前 ARM。固件风扇操作先释放软件会话；心跳检查完整输出进度，超过 3 秒关闭管道并取消后续输出。软件不改固件表，恢复分别尝试 0x69、原自动命令及设置保存回读，保留各阶段错误；恢复失败取消心跳并安排父进程重试。
- 档案新增 fanSoftwareCurves，与两种固件曲线及 fanMode 互斥；捕获、预检、应用、回滚与图形预览共用此契约。
- MSR 负向一步探测已另记 implemented/bug-fix/2026-11-12-negative-voltage-probe.md；实际机器结论仍未知。

## 影响与验证

- 核心软件曲线 5 项、原生 fanctl 16 项、监护 10 项、档案模型 4 项聚焦检查通过；软件会话包括 GPU 休眠/唤醒后丢失、95°C 和输出失败。
- 新增 software_process 5 项，生产 apply/tick/固件切换/恢复/快照与真实监护组合，DCHU 是内存 FFI；验证控制循环停滞、恢复重试、熔断不自动重启、软件档案回滚。通用 watchdog_process 8 项回归通过。均确认自建子进程退出，失败保留本地日志。
- 真实前端加模拟 IPC 的软件曲线 30 项通过，覆盖 2/12 点边界、增删/拖动/数字/键盘、草稿、确认、失败重试、档案图形预览、未知固件禁用及 360–1440px；原有控制台 57 项和 RPM 30 项回归通过。不是 Tauri 真实设备验收。
- npm run build、cargo check --workspace、probe-cli 检查、15 篇 notes 校验及 diff --check 通过。NSIS 重新编译、7z t 完整性通过（1876632B，SHA256 cb42470b1947ce8d7dc5b8097eee78cf3743618978f3fb3c76afe447ac08a8cf），未安装或启动。物理效果保持未验证。

## 风险

同模具不同 BIOS/EC/DLL 仍可能存在协议差异。静态支持不证明物理效果；原厂后台也可能覆写目标。通用 EC 通道的存在不证明任意地址的用途。

## 备选方案

- 继续等待或猜 GPU 0xE8：不采用。
- 循环改写固件两点/十级表模拟 duty：不等价于直接输出且改变持久表，不采用。
- 只把通用 EC 读写接口公开给 UI：缺少领域约束与恢复链路，不采用。

## 依据

- 原厂 FnKey/DCHU.cs 的 RW_ECCMD、Read_ECRAM 与 Write_ECRAM；FanSpeedSetting/FAN.cs 的现有固件分支。
- https://github.com/XiaoQing235/ClevoFanControl/blob/main/src/hardware.rs
- https://github.com/XiaoQing235/ClevoFanControl/blob/main/CONTROL_OWNERSHIP_RESEARCH.md
- https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemfirmwaretable
