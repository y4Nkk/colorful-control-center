# Agent Note: 全局内容展开收起组件

Status: implemented

## 问题

控制台状态、风扇、功耗、GPU 频率与键盘灯光分别拼装六处 details/summary；GPU 两条分支没有诊断样式，折叠入口缺少统一的点击区域、图标与内容间距。用户要求展开收起模块全局组件化并统一接入。

## 决定

- src/components/Collapsible.tsx 唯一拥有内容折叠结构，原生 details/summary 管理开合与键盘行为；调用方传 summary、children 及原生属性。收起不卸载内容，页面 keep-alive 保留开合状态，不增加 store、Host 或依赖。
- src/styles/collapsible.css 唯一拥有折叠样式，由 base.css 全局引入；紧凑点击区域、悬停、焦点与内容间距消费既有令牌。箭头由 Icon 的 chevron-right 语义提供，展开旋转；减少动画设置继续来自全局令牌。
- 六处诊断入口全部接入，删除 ccc-diagnostics 旧样式；check:boundaries 禁止全局组件之外直接拼 details/summary，防止重新出现平行实现。
- 侧栏宽度切换与导航标签显隐继续由 AppShell/SideNav 拥有；保留同期设备组件移入 control 目录及其他现有改动。

## 影响与验证

- npm run build（TypeScript、三页边界检查、Vite）、cargo check 均通过；Rust 只报告增量缓存硬链接降级复制警告。笔记完成后执行 npm run verify-notes。
- 实际 React 页面与隔离模拟 IPC 的 Chromium 检查共 86 项通过：六处源码入口（含 GPU 正常/不可用分支）的点击、Enter/Space、原生辅助功能树展开状态、隐藏内容与键盘序列、焦点环、重渲染与切页保活、诊断内两步确认和单次执行。
- 浅深主题、360/700/1280px、500 字符长诊断、紧凑点击高度、箭头与默认标记、减少动画与普通动效、悬停样式均通过；浅深截图已核对，未捕获 JavaScript 异常。响应式布局检查等待侧栏媒体查询状态更新后取样。
- 边界检查的三项隔离夹具通过：组件外直接 details、summary 均拒绝，使用 Collapsible 接受。旧诊断样式与直接拼装引用已清除。
- 验证脚本、JSON 结果及截图保存在本次 Codex 可视化目录的 collapsible-qa/；未写入真实硬件、用户配置或共享数据。未进行原生 WebView2 窗口视觉验收；本次没有硬件行为或持久化契约变更。

## 备选方案

- 每个模块继续复制结构与 CSS：无法保持唯一所有者，放弃。
- 自建按钮、状态 store 和隐藏容器：重复浏览器已有行为并增加同步成本，放弃。
