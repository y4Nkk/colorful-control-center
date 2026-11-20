# Agent Note: CPU 功耗首写解锁（PL 未上报死锁）

Status: implemented

## 问题

基准机固件未声明 CPUOC（cmd122 bit23=false，OEM CC3 的 `CheckSupportCPUOC` 双条件门控使其从不写 AppSettings page6 33/37），保存值恒 0 被后端过滤为 null；同时 MSR 运行时通道因 PawnIO 设备打开失败不可用。前端 `powerChanges` 以 `plReady` 门控脏检测与滑杆可用性，保存通道的 PL 首写没有任何 UI 入口，档案捕获也只复制 null——三条写入路径全部死锁，PL 滑杆永远显示"未上报"且禁用。

## 决策

首写语义归 `usePowerControl.powerChanges` 唯一所有：PL 保存值未上报时草稿锚定固件出厂默认（`defaultPl1W/defaultPl2W`，来自 WMI18 默认包），脏检测不再依赖上报状态；diff 的 from 如实显示"未上报"。整组提交沿用既有契约（UI 恒成对提交 PL1+PL2，后端 `saved_patches` 本就支持双字段首写，无需改动）。即时目标不受影响：MSR 不可用时整组滑杆已由面板级 `unavailable` 禁用。移除 `plReady` 字段与 PL 滑杆的 `unavailable` 传参；电压滑杆的上报门控（运行时域可独立失败）保留。

## 影响与验证

`tools/ui-check/mock-ipc.mjs` 补齐 `apply_saved_cpu/apply_runtime_cpu/restore_power_profile` 三个缺失命令（真实回写语义：写后回读生效）。新增 `cpu-power-first-write.spec.mjs`：未上报时滑杆可用且锚定 135/135、拖动后预览"未上报 → 120 W / 135 W"整组提交、提交参数精确断言、写后回读显示已上报值；MSR 不可用时首载停在保存页签且即时页签整组禁用。36 条相关既有页面回归与 `npm run build` 通过。`cargo check` 因工作区既有的 `startup/login/model.rs` 未提交改动（lifetime 错误）失败，与本次无关（本次零 Rust 改动）。

真机风险：page6 写入在 CPUOC 能力位关闭的机型上是否被固件/XTU3SERVICE 消费仍属契约 §11.3 未知项——UI 首写只保证"请求已保存"的既有语义，不承诺物理生效；MSR 立即应用通道的恢复依赖 PawnIO 一键提权批复测（§11.7 遗留），不在本次范围。

## 备选方案

由档案编辑器提供手动 PL 字段可绕开捕获死锁，但控制台主入口仍不可用，且会把首写语义分散到两个所有者。
