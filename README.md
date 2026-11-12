# Colorful Control Center（非官方）

面向七彩虹游戏本（蓝天 / 联宝 Uniwill 模具，开发基准机：将星 X15 AT 23，i5-13500HX + RTX 4060 Laptop）的开源硬件控制台，替代 OEM 自带的 Control Center。

> ⚠️ **免责声明**：本项目直接操作嵌入式控制器（EC）与 MSR，写入错误参数可能导致系统不稳定甚至死机（通常重启即恢复）。本项目为社区作品，与七彩虹（Colorful）官方无关。使用风险自负，请勿在重要生产环境使用。

## 功能规划

| 阶段 | 内容 | 状态 |
|---|---|---|
| Phase 0 | 硬件接口探测：OEM WMI 枚举 / EC 寄存器 diff / MSR 锁状态 | 进行中 |
| Phase 1 | 只读监控仪表盘（CPU / GPU / 风扇 / 温度） | 计划 |
| Phase 2 | 自定义风扇曲线（任意控制点 + 滞后回差 + 斜坡限制 + 看门狗） | 计划 |
| Phase 3 | PL1/PL2 功耗墙、偏移电压（undervolt）、GPU 频率偏移、性能档案 | 计划 |
| Phase 4 | 托盘常驻、历史曲线、档案导入导出、键盘灯等 OEM 杂项 | 计划 |

## 技术栈

- **UI**：Tauri 2 + React 19 + TypeScript + Vite
- **硬件层（Rust）**：
  - 内核访问：PawnIO（优先，签名驱动）→ WinRing0（后备）
  - OEM WMI：`wmi` crate（EC 读写 / 性能模式 / 键盘灯）
  - GPU：`nvml-wrapper`（传感器）+ NVAPI FFI（频率偏移）
  - CPU：MSR（PL1/PL2、偏移电压、温度/功耗读数）

详见 [docs/design.md](docs/design.md)。

## 开发

```bash
npm install
npm run tauri dev
```

## 许可

GPL-3.0，见 [LICENSE](LICENSE)。
