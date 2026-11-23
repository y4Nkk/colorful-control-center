# 开发、构建与验证

控制台使用 Tauri 2、React 19、TypeScript、Vite 与 Rust。界面走类型化 IPC，原生层按设备能力使用 InsydeDCHU、PawnIO、NVML 和 NVAPI。

## 开发环境

从 Windows x64 仓库根目录执行，准备 Node.js 24 和 Rustup。仓库通过 `rust-toolchain.toml` 选择 Windows GNU 工具链。运行桌面窗口需要 WebView2。

```powershell
npm ci
npm run build
cargo check --workspace --locked
npm run verify-notes
```

前端构建会检查类型、组件与 IPC 边界和桌面图标资产。修改 `src-tauri/icons/icon.svg` 后，运行 `npm run generate:icons` 重新生成桌面图标和浏览器图标。

在允许操作硬件的开发机上，可用 `npm run tauri dev` 启动完整控制台。验证 UI 时可直接使用下文的模拟宿主。

## 构建 Windows 安装包

安装包构建另需 PowerShell 7、7-Zip 命令行、MSBuild 和 .NET Framework 4.8.1 WPF targeting pack。

```powershell
npm run build:installer
```

- 对外安装包位于 `target/release/bundle/installer/ColorfulControlCenter_<版本>_x64-setup.exe`。在同目录的 `verification.json` 查看版本、大小和安装包、NSIS 引擎、内嵌主程序的 SHA-256。
- Tauri 生成的 NSIS 引擎位于 `target/release/bundle/nsis/`。WPF 向导封装该引擎，发布前核对五份内嵌资源和实际内嵌主程序。替换产物失败时恢复上一份安装包及校验记录。
- 通过 `npm run check:installer` 在隔离夹具中检查安装、卸载、失败重试及摘要校验。日志与截图保存在 `target/installer-check/`。正式包安装、WebView2 准备和更新路径另有 [真机验收清单](acceptance-checklist.md)。
- 安装向导实现见 [installer](../installer/)，插画来源见 [SOURCES.md](../installer/assets/SOURCES.md)。

### 1.0.0 发布核对

正式首发的标签为 `v1.0.0`。Windows 版本资源和安装包名取自 `src-tauri/tauri.conf.json`。应用的 npm 与 Rust 清单及锁文件采用相同版本号。

1. 构建后核对 `target/release/bundle/installer/verification.json` 的版本为 `1.0.0`，安装包为 `ColorfulControlCenter_1.0.0_x64-setup.exe`。用 `Get-FileHash -Algorithm SHA256` 核对最终文件与记录一致。
2. 用同一个候选包按 [真机验收清单](acceptance-checklist.md) 记录安装、更新、卸载和硬件验证结果。隔离检查结果随候选包记录。
3. 推送 `v1.0.0` 标签触发 [Windows installer 工作流](../.github/workflows/windows-release.yml)。标签、应用版本和 [更新日志](../CHANGELOG.md) 条目一致时，GitHub Actions 构建 Windows x64 向导，发布安装包、`SHA256SUMS.txt` 和同版本的更新说明。

可在 GitHub Actions 手动运行 **Windows installer** 下载验证用安装包。手动构建产物在运行详情的 Artifacts 中。标签构建在发布作业中重新核对摘要，已有同名 Release 由维护者管理。

### 应用内更新发布

- 仓库变量 `TAURI_UPDATER_PUBLIC_KEY` 将项目公钥嵌入 Windows 主程序。标签构建用仓库密钥 `TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 为最终 WPF 安装包签名。私钥及解锁信息由维护者保管。
- 发布同时上传安装包、`.sig`、`SHA256SUMS.txt` 和 `latest.json`。应用使用 `releases/latest/download/latest.json`。清单中的更新说明取对应版本的 CHANGELOG，签名绑定版本和安装包内容。
- 未配置公钥的本地构建可通过应用内的 GitHub 发布页入口手动更新。验收覆盖升级时，先安装支持更新的旧版，再从「设置 → 关于」检查、取消、重新下载并确认升级，核对退出恢复、安装路径、启动后的版本以及已有偏好。

## 隔离验证

| 命令 | 验证范围 |
| --- | --- |
| `npm run check:controls` | 控件所有权、样式边界与下拉菜单定位。 |
| `npm run check:ui -- navigation.spec.mjs` | 用 Windows Edge 和模拟 IPC 验证实际 React 工作区。按改动替换测试文件。 |
| `npm run check:ui:native` | 用独立 WebView2 宿主验证原生渲染。首次构建需要已缓存的 Rust 依赖，产物在 `target/ui-webview2-build/`。 |
| `cargo test -p ccc-core --lib` | 共享领域逻辑。 |
| `cargo test -p colorful-control-center --lib fanctl:: -- --test-threads=1` | 风扇域。按改动选择 `watchdog::`、`powerctl::`、`profiles::`、`storage::` 或 `startup::` 等过滤条件。 |

浏览器和 WebView2 检查都使用本地 1450 端口，WebView2 宿主还需要 9450 端口。分别运行，浏览数据、日志与截图保存在 `target/ui-check/`。

`tools/control-sim` 将生产领域逻辑接到内存传输和私有临时状态，通过子进程、管道和控制权租约验证故障与恢复。根据改动选择单个入口。

```powershell
cargo test --locked --manifest-path tools/control-sim/Cargo.toml --test watchdog_process -- --test-threads=1
```

可选入口包括 `controller_lease_process`、`storage_process`、`software_process`、`display_process`、`gpu_power_process`、`gpu_session_process` 和 `lan_authorization_process`。模拟进程结果和真机效果分别记录。

### README 截图

发布文档图片位于 [docs/screenshots](screenshots/)。刷新时使用当前 `npm run build` 产物，在无外部网络、无设备映射的 Linux 容器内用 Chromium 无头模式打开静态前端。在加载应用前注入 [模拟 IPC](../tools/ui-check/mock-ipc.mjs)，检查传感读数、风扇曲线、键盘预览和电池状态已渲染，再导出各工作区的 PNG。截图中的读数、机型信息及配置均来自隔离夹具。1.0.0 截图使用容器的文泉驿正黑与文泉驿等宽正黑。

## 硬件探测与契约

- `cargo build --release --locked --manifest-path tools/probe-cli/Cargo.toml` 生成 `tools/probe-cli/target/release/ccc-probe.exe`。运行前可用 `--help` 查看参数。批量探测默认只读，电压写入测试需显式传入 `-ExecuteVoltageTest`。[原厂程序集核对](../tools/research/README.md) · [EC 离线比对](../tools/probe/ec-diff-guide.md)
- 硬件传输、恢复事务与验证范围见 [硬件架构](design.md)。接口细节见 [DCHU](oem-dchu-contract.md)、[原厂设备](oem-device-contract.md)、[NVAPI](nvapi-contract.md)、[键盘](keyboard-contract.md)、[电池](battery-contract.md)、[触摸板](touchpad-contract.md)、[麦克风](microphone-contract.md)、[屏幕亮度](screen-brightness-contract.md) 和 [供电档案规则](power-profile-policy-contract.md)。
- 页面结构和组件所有权见 [DESIGN.md](../DESIGN.md)，接入及提交前检查见 [AGENTS.md](../AGENTS.md)。
