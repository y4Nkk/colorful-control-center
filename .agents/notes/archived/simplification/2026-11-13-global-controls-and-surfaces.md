# Agent Note: 全局表单控件与表面样式

Status: implemented

## 问题

用户要求下拉框、复选框、输入框、拖动条、按钮及边框圆角样式全局组件化并统一接入。现有 Button/Input 仅为 CSS 种子，原生元素与控件样式散落业务目录，曲线数值输入等另有覆盖，无法保持唯一所有者。

## 决定

- components 平铺实现 Button、Input、Select、Combobox、Checkbox、Radio、Slider、Surface；使用原生控件与 React 属性/ref 透传，不增加依赖、Provider、store 或业务状态。
- Button 统一主次操作、紧凑操作、图标、导航、选择、曲线拖动点与颜色键帽，尺寸/圆角/禁用/危险态由 button.css 拥有；ConfirmButton 只保留两步确认行为，危险样式由 Button tone 表达。选中状态不被普通悬停态覆盖。
- Input 负责文本、搜索、数字、颜色与隐藏文件输入，Select 保留原生菜单；字体选择的搜索、弹层、键盘与选项交互迁入全局 Combobox，FontPicker 只保留字体读取及业务回调。field.css 唯一拥有字段样式，原生菜单跟随浅深 color-scheme。
- Checkbox 接入键盘分区草稿开关，Radio 接入现有单选与卡片；软件即时异步开关继续使用 Switch。Slider 拥有原生 range 及 slider.css，ScaleSlider 只组合读数、单位和当前值标记。
- Surface 与 surface.css 统一透明布局、面板、卡片、信息组、工作区与内嵌区；滚动容器复用 Surface，工作区明确传 workspace，局部滚动保持透明。业务 CSS 保留布局、图形和内容，移除控件外观及容器边框圆角覆盖。
- tokens.css 唯一拥有共享边框、焦点和滑杆/拖动点尺寸；DESIGN.md 登记所有者，base.css 聚合样式。check:boundaries 拒绝业务直接拼 button/select/input 及覆盖基础控件类。
- 保留同期预设模式插画、系统概览和软件设置接入；不修改 IPC、硬件行为或持久化语义。

## 影响与验证

- npm run build（TypeScript、三页边界检查、Vite）、cargo check 均通过；Rust 仅报告增量缓存硬链接降级复制警告。生命周期转换后再运行 npm run verify-notes。
- 实际 React 页面、隔离模拟 IPC 的 headless Chromium 检查 132 项通过：三页、文本/数字/颜色/隐藏文件输入、复选框与单选、普通与搜索下拉、滑杆键盘及鼠标拖动、曲线键盘及指针捕获、两步确认危险态、RPM 与软件曲线、120 个逐键颜色按钮、草稿保活和导入预览。
- 三页及 RPM/逐键分支在浅深主题、360/700/1280px 下无控件横向溢出；字段 40/32px 高与 12px 圆角、原生菜单配色、选中项悬停保色与键盘焦点均核对。截图已检查，未捕获 JavaScript 异常。
- 搜索下拉高度最后收敛后，仅补跑 6 项定向检查：长列表限高/滚动、过滤后按内容缩高、共享卡片边框、选择/退出后焦点恢复，全部通过；不重复未受影响的三页检查。
- 8 项隔离边界夹具通过，包含原生元素/业务样式覆盖拒绝、原语组合/原生所有者/全局样式接受。源码文件和函数长度、冲突标记、旧引用及相关差异检查通过。
- 验证脚本、结果与截图保存在本次 Codex 可视化目录 controls-qa/。首次完整 Chromium 截图进程退出，改用独立 headless-shell 后完成；未进行原生 WebView2 窗口视觉验收。全部写入请求由模拟 IPC 接收，未写真实硬件、用户配置或共享业务数据。

## 备选方案

- 只集中 CSS，不建立组件和接入约束：继续允许模块绕过统一入口，放弃。
- 引入完整 UI 框架或重做硬件工作流：超出需求且增加依赖，放弃。

## 被取代

本笔记已由 implemented/simplification/2026-11-13-global-control-contract-closure.md 取代。原生 Select、Combobox 及旧边界描述仅保留为历史，不再是当前契约。
