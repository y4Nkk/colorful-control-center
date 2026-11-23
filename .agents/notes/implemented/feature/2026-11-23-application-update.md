# Agent Note: 应用内检查更新

Status: implemented

## 问题

关于页只能读取本机版本。用户需要检查 GitHub 新版本并从应用内取得安全的升级入口。

## 决定

- 参考 Agent Switchboard 的 Tauri Updater 与 GitHub Releases `latest.json`，由原生端拥有已检查版本、下载取消和签名验证。清单版本须与签名中的版本一致。
- 更新载荷使用已经发布给用户的 Windows 安装向导。下载确认后由向导继续处理同目录覆盖安装、程序占用、正常退出及升级后的启动。
- 标签发布用 Colorful Control Center 专用签名密钥生成清单和签名，正文保留下载表格，清单更新说明来自同一版 CHANGELOG。

## 影响与验证

- 检查新版本、已是最新版、失败重试、取消和安装向导交接。
- 在隔离环境验证签名、版本绑定和发布清单。真机覆盖升级另行按验收清单记录。

## 备选方案

- 直接调用 Tauri Updater 的 Windows 安装逻辑会绕过控制台现有的退出与恢复协作。使用已校验的 WPF 安装向导完成交接。
