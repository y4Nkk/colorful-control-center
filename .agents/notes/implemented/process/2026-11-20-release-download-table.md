# Agent Note: Release 页下载表格

Status: implemented

## 问题

v1.0.1 的 GitHub Release 正文只有 CHANGELOG 节选和一行 SHA-256（已通过 API 核验线上 body），下载入口只剩页面底部的 Assets 列表。参考 `F:/projects/agent-switchboard` 的实现（`.github/release_template.md` 模板 + workflow 内 sed 替换占位符后拼接），为本项目补齐下载表格并按单平台场景优化。

## 决定

- 新增 `.github/release_template.md`：下载量徽章 + 「按你的系统下载」表格（Windows x64 安装包徽章 + SHA256SUMS 校验和徽章）+ 完整变更列表链接 + 分隔线。占位符 `REPOSITORY` / `VERSION` / `PRODUCTNAME`。
- `scripts/stage-windows-release.ps1` 在 CI 环境替换占位符并把表格前置到发布正文。新增应用内更新后，同一脚本仍以不含下载表的 CHANGELOG 节选生成 `latest.json`，见 [应用内检查更新](../feature/2026-11-23-application-update.md)。
- 表格放正文顶部而非 agent-switchboard 的底部：本项目单平台，下载优先于平台导航。所有徽章统一 `flat-square` 风格（agent-switchboard 的表格徽章是默认圆角，与其自身下载量徽章不一致）。
- `PRODUCTNAME` 占位符从 `tauri.conf.json` 注入：安装包文件名契约仍由 tauri.conf.json 唯一拥有，模板不复写该事实；`VERSION` 同理由脚本注入。表格链接的资产名与 workflow 上传清单（installer + SHA256SUMS.txt）一致。
- 完整变更列表锚定 tag（`blob/vVERSION/CHANGELOG.md`），而非 agent-switchboard 的分支链接，发布页指向不可变快照。
- 无 CI 环境变量时（本地 staging）不拼表格，与既有相对链接重写的行为一致：本地无法构造正确的绝对下载地址。

## 备选方案

- 照搬 agent-switchboard 的 workflow 独立拼接步骤：被否决。脚本已拥有 CHANGELOG 节选，可以直接生成发布正文和供应用显示的说明。
- 表格放正文底部（agent-switchboard 布局）：被否决。多平台项目表格是平台导航放底部合理；本项目单平台，下载是发布页的第一诉求。
- 发布后手动 `gh release edit` 补表格：被否决。人肉步骤会漂移，模板进仓库由脚本唯一驱动才是单一来源。
- 模板内硬编码安装包文件名（agent-switchboard 的做法）：被否决。文件名契约归 tauri.conf.json 所有，用 `PRODUCTNAME` 占位符注入，避免两处维护。

## 影响与验证

- pwsh 模拟脚本复用脚本内的同一段替换逻辑（CHANGELOG 提取 + 模板替换 + 拼接）渲染 1.0.1 正文：占位符全部消除，链接指向 `y4Nkk/colorful-control-center` / `v1.0.1`。
- `npm run verify-notes` 通过。改动不触及 TS/Rust，`npm run build` 与 `cargo check` 不受影响。
- 表格在真实 Release 页的最终渲染（shields.io 徽章中文 label、下载计数需 release 存在后才计数）留待下一个标签推送后目检。

## 风险

shields.io 为第三方服务，徽章可能加载失败；失败时表格退化为可点击的 Markdown 文本链接，下载不受阻。下载计数徽章在新建 release 后短暂显示 0 属正常。
