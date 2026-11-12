# Colorful Control Center（非官方）

面向七彩虹游戏本（蓝天 / 联宝 Uniwill 模具，开发基准机：将星 X15 AT 23，i5-13500HX + RTX 4060 Laptop）的开源硬件控制台，替代 OEM 自带的 Control Center。

> ⚠️ **免责声明**：本项目直接操作嵌入式控制器（EC）与 MSR，写入错误参数可能导致系统不稳定甚至死机（通常重启即恢复）。本项目为社区作品，与七彩虹（Colorful）官方无关。使用风险自负，请勿在重要生产环境使用。

## 功能规划

| 阶段 | 内容 | 状态 |
|---|---|---|
| Phase 0 | 硬件接口探测：OEM WMI 枚举 / EC 寄存器 diff / MSR 锁状态 | 计划 |
| Phase 1 | 只读监控仪表盘（CPU / GPU / 风扇 / 温度） | 计划 |
| Phase 2 | 自定义风扇曲线（任意控制点 + 滞后回差 + 斜坡限制 + 看门狗） | 计划 |
| Phase 3 | PL1/PL2 功耗墙、偏移电压（undervolt）、GPU 频率偏移、性能档案 | 计划 |
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
| [docs/design.md](docs/design.md) | 硬件控制通道架构与 Phase 0 清单唯一所有者 |
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
