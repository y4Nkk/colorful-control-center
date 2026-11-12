# 更新日志

本文件是版本历史的唯一来源（Keep a Changelog 格式）。

## [Unreleased]

### 新增

- 全局底座：设计令牌（tokens.css 唯一所有者）、IPC 类型化边界（api/client.ts）、全局通知（toast 单例 + Toaster Host）、AppShell 外框、页面注册表与 5 个模块接入点（keep-alive 工作区）、系统主题跟随
- Rust 侧：集中命令注册表（command_registry.rs）+ 命令薄层（commands/）+ `get_backend_info` 通路验证命令
- 可执行规范：`check:boundaries`（IPC / 令牌 / 页面接线三边界）、`verify-notes`（决策笔记结构与格式）
- 文档体系：DESIGN.md 设计契约、AGENTS.md 协作规范与全局化接入契约、本更新日志、README 文档索引表
- 决策笔记底座：`.agents/notes/` 三态目录 + 首篇笔记（底座技术契约）

### 变更

- `docs/design.md` §6「UI 方向」移交 DESIGN.md（原节改为指针）
- 移除 Tauri 模板演示代码（greet 命令、App.css、模板图标）
