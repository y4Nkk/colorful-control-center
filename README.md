<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="112" height="112" alt="Colorful Control Center 五叶风扇标识">
</p>

<h1 align="center">Colorful Control Center</h1>

<p align="center">七彩虹游戏本的非官方 Windows 控制台</p>

<p align="center">
  <a href="CHANGELOG.md">更新日志</a> · <a href="docs/development.md">源码构建</a> · <a href="https://github.com/y4Nkk/colorful-control-center/releases">GitHub Releases</a>
</p>

在一个窗口里查看温度和风扇转速，调整性能、键盘灯光与充电策略，并把常用配置保存为档案。项目以七彩虹将星 X15 AT 23（Core i5-13500HX、GeForce RTX 4060 Laptop）为开发基准。可用设置随蓝天或联宝机型、固件和驱动能力变化。

## 界面预览

下图使用 1.0.0 前端，在断网 Docker 容器内由 Chromium 渲染。传感器读数和设备状态均为模拟数据。

![系统概览，显示当前预设、五分钟温度趋势、CPU 和 GPU 读数](docs/screenshots/system-overview.png)

| 性能与散热 · 风扇设置 | 键盘与灯光 |
| --- | --- |
| ![风扇设置的固件曲线编辑器](docs/screenshots/fan-control.png) | ![键盘与灯光页的三区 RGB 预览](docs/screenshots/keyboard-lighting.png) |

![电源页的电池电量、充电保护和节能时段](docs/screenshots/power.png)

## 功能

| 工作区 | 可以做什么 |
| --- | --- |
| **概览** | 查看当前模式、温度、转速、内存与存储。 |
| **性能与散热** | 切换原厂预设，调整风扇模式、偏移和曲线，调节 CPU 功耗、电压和 GPU 频率。 |
| **键盘与灯光** | 按设备能力设置分区或逐键 RGB、外围灯区和闲置关灯。管理按键映射、宏，以及授权接管的原厂热键与屏显。 |
| **电源与档案** | 管理电池、节能、供电和 USB 选择性暂停。按组保存、预览、导入或导出配置档案，并设置启动、插电、电池和唤醒规则。 |
| **设备配置** | 设置刷新率、主屏方向和显卡模式，管理独显电源、触摸板、麦克风及飞行模式。显卡切换可在重启后核对结果。 |
| **设置与托盘** | 选择普通或管理员登录自启，以及启动时显示主界面或最小化到托盘。启动时默认结束已核验的原厂 FnKey，可在设置中关闭。通过托盘切换受支持的模式、控制风扇、打开档案预览或退出。 |

在固件支持的设备上，「性能与散热」的娱乐档和性能档都可分别开关 **Turbo 风扇**、**动态调校 DTT** 和**寂静模式**。DTT 需要对应的驱动就绪，单独调整时保留当前风扇曲线。从自定义固件曲线开启娱乐档 Turbo 时，界面会先确认切换到自动风扇，原曲线可恢复。

## 安装与使用

**当前版本为 1.0.3。** Windows x64 安装包名为 `ColorfulControlCenter_1.0.3_x64-setup.exe`，由 [GitHub Releases](https://github.com/y4Nkk/colorful-control-center/releases) 提供。源码构建方式见 [开发与构建指南](docs/development.md)。

1. 运行安装包，按向导为当前用户安装。向导需要 .NET Framework 4.8.1，并会在需要时联网准备 WebView2。[.NET Framework 系统要求](https://learn.microsoft.com/dotnet/framework/get-started/system-requirements)
2. 打开「概览」，确认可读的硬件状态。原厂硬件通道依赖机型已有的 InsydeDCHU 组件和驱动，部分 CPU 实时调校还需要 PawnIO。
3. 在「性能与散热」直接切换风扇模式，完成曲线点位或加速偏移的调整后自动应用。其他模块按界面提示提交，刷新率和主屏方向可在 15 秒试用内保留或撤回。硬件调校会影响散热和稳定性，请先确认当前值与恢复入口。
4. 在「电源与档案 → 配置档案」选择「保存当前配置」，为读取的设置命名并选定配置组。应用前核对当前值、档案目标和设备联动。在同页的「电源设置」配置启动、供电或唤醒规则，关联档案更新后在那里重新确认。
5. 更新或卸载前，从托盘菜单退出控制台，让正在执行的事务完成并释放临时硬件控制。更新向导也提供「关闭并安装」。

整机支持与物理效果以逐项真机验收为准。[真机验收清单](docs/acceptance-checklist.md)

## 文档

- [开发、构建与隔离验证](docs/development.md)
- [硬件架构与恢复语义](docs/design.md)
- [界面设计与组件契约](DESIGN.md)
- [版本历史](CHANGELOG.md)
- [问题反馈](https://github.com/y4Nkk/colorful-control-center/issues)

## 许可证

GPL-3.0。[LICENSE](LICENSE)
