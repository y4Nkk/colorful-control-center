# 截图与演示数据

本目录的 11 张截图于 2026 年 11 月 27 日从 2.0.0 生产前端与 Windows 安装向导截取，供简体中文和英文 README 共用。画面为简体中文。 [中文展示](../../README.md#界面预览) · [English gallery](../../README.en.md#screenshots)

## 展示内容

| 图片 | 内容 |
| --- | --- |
| [system-overview.png](system-overview.png) | 娱乐模式渐变背景、CPU 与 GPU 读数和灰底温度趋势 |
| [overview-capacity.png](overview-capacity.png) | 内存使用率和灰底趋势图，三个不同占用档位的磁盘卷 |
| [performance.png](performance.png) | 当前预设、CPU 功耗与电压和 GPU 频率调校 |
| [fan-control.png](fan-control.png) | 自动档的整机策略、曲线编辑入口和两路转速表盘 |
| [fan-curve.png](fan-curve.png) | 深色主题下从寂静档打开的风扇曲线编辑器 |
| [keyboard-lighting.png](keyboard-lighting.png) | 键盘分区光效、色轮及三色背光预览 |
| [power.png](power.png) | Windows 电源计划、充电范围、节能计划和档案应用 |
| [profiles.png](profiles.png) | 内置与个人档案列表，安静办公的当前值和目标值 |
| [device-configuration.png](device-configuration.png) | 屏幕显示、自动刷新率、显卡模式与独显电源 |
| [settings.png](settings.png) | 主题、字体、缩放、动效、窗口和托盘设置 |
| [installer.png](installer.png) | Windows 安装向导、版本及模拟安装位置 |

## 数据来源

桌面界面使用 `npm run build` 生成的前端，在独立 Chromium 上下文中加载。温度、转速、内存、磁盘、电量、显示状态和档案都来自模拟 IPC。请求仅走本机回环地址，模拟状态保存在浏览器内存中。安装向导使用隔离渲染工具和模拟安装位置。 [模拟 IPC 夹具](../../tools/ui-check/mock-ipc.mjs) · [安装向导验证](../../tools/installer-check/check.ps1) · [真机验收清单](../acceptance-checklist.md)

## Capture notes

The 11 screenshots show the Simplified Chinese interface from the 2.0.0 production frontend and Windows setup wizard, captured on November 27, 2026. Both README files use these images. An isolated Chromium context reads simulated hardware and profiles over mock IPC. Requests stay on the local loopback origin, and demo state stays in browser memory. The setup wizard uses an isolated render and a sample installation path. [Mock IPC fixture](../../tools/ui-check/mock-ipc.mjs) · [Installer checks](../../tools/installer-check/check.ps1) · [Hardware acceptance checklist](../acceptance-checklist.md)
