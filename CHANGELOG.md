# 更新日志

本文件是版本历史的唯一来源（Keep a Changelog 格式）。

## [Unreleased]

### 新增

- **Phase 0 硬件接口探测（0.1–0.4）**：OEM WMI 枚举脚本与结果存档、nbfc-linux 同模具配置检索、
  docs/oem-dchu-contract.md（基准机 OEM Control Center 逆向审查得到的完整硬件接口契约）、
  probe-cli（DCHU 能力报告 / CPUID / PawnIO MSR 锁检测与 0x150 降压写入测试）、
  tools/probe/run-admin-probe.ps1（一次 UAC 的提权验证批）
- **Phase 1 监控 MVP**：HAL 层（InsydeDCHU FFI + NVML，回声检测、NVML 交叉验证告警）、
  1Hz hardware-snapshot 事件推送、监控仪表盘真实读数（CPU/GPU/风扇/内存 + FactList 原语）
- **Phase 2 风扇曲线（OEM 曲线模式）**：`ccc-core` 纯逻辑曲线引擎（插值/滞后/斜坡/熔断，11 项单元测试）、
  cmd14 曲线写入 + cmd121 模式切换/偏移/恢复默认 + page4 持久化、风扇页曲线编辑器
  （SVG 预览 + 二次确认 + 失败 toast）
- 全局底座：设计令牌（tokens.css 唯一所有者）、IPC 类型化边界（api/client.ts）、全局通知（toast 单例 + Toaster Host）、AppShell 外框、页面注册表与 5 个模块接入点（keep-alive 工作区）、系统主题跟随
- Rust 侧：集中命令注册表（command_registry.rs）+ 命令薄层（commands/）+ `get_backend_info` 通路验证命令
- 可执行规范：`check:boundaries`（IPC / 令牌 / 页面接线三边界）、`verify-notes`（决策笔记结构与格式）
- 文档体系：DESIGN.md 设计契约、AGENTS.md 协作规范与全局化接入契约、本更新日志、README 文档索引表
- 决策笔记底座：`.agents/notes/` 三态目录

### 变更

- 架构修订：硬件主通道改为 InsydeDCHU.dll FFI（OEM 已装签名驱动），常规路径不再自带内核驱动；
  本机无 MSVC Build Tools，仓库经 rust-toolchain.toml 固定 windows-gnu 工具链
- `docs/design.md` §6「UI 方向」移交 DESIGN.md（原节改为指针）
- 移除 Tauri 模板演示代码（greet 命令、App.css、模板图标）
