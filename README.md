# Colorful Control Center（非官方）

面向七彩虹游戏本（蓝天 / 联宝 Uniwill 模具，开发基准机：将星 X15 AT 23，i5-13500HX + RTX 4060 Laptop）的开源硬件控制台，替代 OEM 自带的 Control Center。

> ⚠️ **免责声明**：本项目直接操作嵌入式控制器（EC）与 MSR，写入错误参数可能导致系统不稳定甚至死机（通常重启即恢复）。本项目为社区作品，与七彩虹（Colorful）官方无关。使用风险自负，请勿在重要生产环境使用。

## 功能规划

| 阶段 | 内容 | 状态 |
|---|---|---|
| Phase 0 | 硬件接口探测 | ✅ 0.1–0.4 完成（[oem-dchu-contract](docs/oem-dchu-contract.md)）；提权验证批：`tools/probe/run-admin-probe.ps1` |
| Phase 1 | 只读监控仪表盘（CPU / GPU / 风扇 / 温度 + 5 分钟历史曲线） | ✅ 已实现（待真机对齐 HWiNFO 验收） |
| Phase 2 | 自定义风扇曲线（OEM 曲线模式：固件执行 + 二次确认 + ≥20% 钳位；软件闭环 + 看门狗为后续增强） | ✅ 已实现（待真机写入验证） |
| Phase 3 | PL1/PL2 功耗墙、偏移电压（page6 持久化 + MSR 运行时双通道）、性能档案（JSON + 一键应用 + 导入导出） | ✅ 已实现（undervolt 结论待 MSR 锁检测） |
| Phase 4 | 托盘常驻（关闭到托盘）、设置页（主题/自启/关于） | ✅ 已实现；键盘 RGB 待验证（契约 §11-6） |
| 底座 | 全局底座与规范（令牌 / IPC 边界 / 全局通知 / 页面接入点 / 可执行规范） | ✅ 已完成 |

## 技术栈

- **UI**：Tauri 2 + React 19 + TypeScript + Vite（零依赖极简底座：无路由库 / 无状态库 / 无 CSS 框架）
- **硬件层（Rust）**：
  - 内核访问：PawnIO（优先，签名驱动）→ WinRing0（后备）
  - OEM WMI：`wmi` crate（EC 读写 / 性能模式 / 键盘灯）
  - GPU：`nvml-wrapper`（传感器）+ NVAPI FFI（频率偏移）
  - CPU：MSR（PL1/PL2、偏移电压、温度/功耗读数）

## 项目文档

| 文档 | 事实归属 |
|---|---|
| [README.md](README.md) | 产品事实（定位、功能规划、技术栈、构建）唯一所有者 |
| [DESIGN.md](DESIGN.md) | UI / 交互设计契约唯一所有者 |
| [AGENTS.md](AGENTS.md) | 协作规范与全局化接入契约唯一所有者 |
| [CHANGELOG.md](CHANGELOG.md) | 版本历史唯一来源 |
| [docs/design.md](docs/design.md) | 硬件控制通道架构与安全设计唯一所有者 |
| [docs/oem-dchu-contract.md](docs/oem-dchu-contract.md) | OEM InsydeDCHU 硬件接口契约（逆向事实）唯一所有者 |
| [docs/acceptance-checklist.md](docs/acceptance-checklist.md) | 真机验收清单（一键提权批 + 目检步骤） |
| [.agents/notes/](.agents/notes/) | 架构决策笔记（三态流转，`npm run verify-notes` 校验） |

## 开发

```bash
npm install
npm run tauri dev     # 桌面应用
npm run build         # tsc + 边界检查 + vite build
npm run verify-notes  # 决策笔记格式校验
```

## 许可

GPL-3.0，见 [LICENSE](LICENSE)。
