# Agent Note: 原厂充电保护与电池读数

Status: implemented

## 问题

电池管理尚无领域和界面。已静态审查本机CC3 Window1.Load_Preference/SetFlexiCharger及已安装BatteryPackUtility源码；本机DSDT PK04 Case0x1E/0x1F印证充电策略读写。校准由原厂应用推进两轮充放电并切换AC及FlexiCharge，不能当作一个可回读布尔开关。

## 决定

- battery领域拥有充电保护即时策略、固件返回的起止候选、预览修订、首次基线和失败补偿。cmd4/sub30读取，sub31携带enabled/start/stop；不额外改CC40专有保存字段或原厂注册表。
- 候选来自响应16..31/32..47非零值，不硬编码70/80或通用滑杆。拒绝未知标志、非法百分比、起止倒置、旧预览及候选变化；原始关闭状态的零阈值可精确恢复。
- 电量、AC、充电与剩余时间来自GetSystemPowerStatus；unknown保留null。能力页v1.0的WMI16位图13..14 bit9声明BatteryUtility才读取cmd7诊断，原始容量单位未证明时只计算满充/设计比例，不冒充mWh。
- UI在控制台用现有Surface、Select、Switch、FactList和比例读数组合；草稿、确认和恢复分离，错误保留草稿。校准及EnergySave属于同一电池管理目标的后续接入，当前不伪造完成状态或触发隐藏命令。
- 不启动原厂软件、不执行真实DCHU或电源操作，所有写入检查注入内存传输；后续长时校准必须另有进度、停止、进程退出与恢复机制。

## 影响与验证

新增battery领域、3项IPC与控制台卡片，协议见docs/battery-contract.md。16项原生内存检查通过，覆盖候选约束、能力与旧修订、无电池、未知值、部分失败、回读不符、补偿失败可重试和原始关闭状态恢复。4项页面隔离IPC检查通过，含草稿确认重置、错误保留、恢复与360px布局；截图复核后为起止选择补充可见标签并重新通过4项检查。npm build、cargo check --offline --workspace和25项控件检查通过。未运行真实电源查询或DCHU命令。

## 风险

固件策略回读不证明真实充电电流或停止阈值效果。Windows汇总电池不提供物理电池身份，容量比例不等于厂商健康诊断；原厂长时校准和节能计划仍需继续实现。

## 备选方案

- 固定通用百分比：与固件返回范围不一致，拒绝。
- 只保存UI偏好：无法控制机器或证明固件接受，拒绝。
- 从旧整型导出的回声推断BatteryUtility能力：已有无效证据，拒绝。
