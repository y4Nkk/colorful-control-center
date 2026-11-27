# Agent Note: 原厂充电保护与电池读数

Status: implemented

## 问题

电池管理尚无领域和界面。已静态审查本机CC3 Window1.Load_Preference/SetFlexiCharger及已安装BatteryPackUtility源码；本机DSDT PK04 Case0x1E/0x1F印证充电策略读写。校准由原厂应用推进两轮充放电并切换AC及FlexiCharge，不能当作一个可回读布尔开关。

## 决定

- battery 领域拥有充电保护即时策略、固件返回的起止候选、修订和失败补偿。cmd4/sub30 读取，sub31 携带 enabled/start/stop。写入范围仅限当前充电策略。
- 候选来自响应 16..31 和 32..47 的非零值。拒绝未知标志、非法百分比、起止倒置、旧修订及候选变化。失败时恢复本次写入前的值，包括原始关闭状态的零阈值。
- 电量、AC、充电与剩余时间来自GetSystemPowerStatus；unknown保留null。能力页v1.0的WMI16位图13..14 bit9声明BatteryUtility才读取cmd7诊断，原始容量单位未证明时只计算满充/设计比例，不冒充mWh。
- 界面首读用共享 Loading，获得结果后显示电量、供电及充电状态。开关和有效范围直接提交和回读，空阈值等待用户选出合法组合。节能计划与校准由各自模块管理。
- 2026-11-27 移除初始恢复入口和首次基线，充电 IPC 收敛到读取与应用。详见 implemented/simplification/2026-11-27-battery-charge-restore-entry.md。
- 2026-11-27 将充电保护和 Windows 电量的读取失败原因分开返回，移除未使用的 Windows 节电标志。供电确认接通后才按系统充电标志、满电读数与原厂充电范围展示状态。真机验收由 `ccc-probe charge` 完成，默认只读，显式执行时回读并恢复原值。

## 影响与验证

协议见 docs/battery-contract.md。2026-11-27 复验通过 192 项电池相关原生测试和 2 项充电探针测试。模拟界面覆盖真实原因、无电池、未知状态、阈值写入失败、切页和宽窄布局。普通用户权限的真机记录覆盖写入 60%–80%、回读、恢复原值和延迟复核，见 tools/probe/charge-acceptance-2026-11-27.json。

## 风险

真机验收范围是固件策略写入、回读和恢复。实际停充与重启后保持需要独立记录。Windows 只提供汇总电池，设备身份及 OEM 容量单位的边界见 docs/battery-contract.md。

## 备选方案

- 固定通用百分比：与固件返回范围不一致，拒绝。
- 只保存UI偏好：无法控制机器或证明固件接受，拒绝。
- 从旧整型导出的回声推断BatteryUtility能力：已有无效证据，拒绝。
