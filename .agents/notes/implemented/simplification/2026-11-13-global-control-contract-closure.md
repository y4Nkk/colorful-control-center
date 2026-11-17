# Agent Note: 全局控件契约与选择菜单完整收敛

Status: implemented

## 问题

2026-11-13-global-controls-and-surfaces 完成多数入口迁移后，普通 Select 仍输出原生 select/option，搜索菜单另用 Combobox，展开面不统一且缺少边缘定位；样式边界未覆盖 Surface、子部件及业务别名。用户明确要求完整、无兼容、无过渡、无技术债实现全局化组件并统一接入。本笔记取代已归档的旧笔记；折叠组件原有笔记继续有效。

## 决定

- 全局原语统一覆盖 Button、Input、Select、Checkbox、Radio、SegmentedControl、Slider、Surface、Switch、ScrollArea 与 Collapsible。所有业务入口直接迁移；边框、圆角、尺寸、状态和焦点只由全局样式及 tokens.css 拥有，业务保留布局、内容和行为。
- Select 只有受控 value/options/onChange(value) 当前契约，普通与搜索选择共用一个实现。删除 Combobox、原生 select/option、children/event 契约、ccc-combobox 及旧原生选择样式；字体读取仍由 FontPicker 拥有。
- SelectMenu 挂 body，复用 Button/Input/Surface/ScrollArea；use-select 统一键盘、搜索、禁用、关闭和焦点，use-select-position 及纯函数 select-layout 统一上下展开、边缘避让、限高、滚动跟随与根缩放换算。搜索框在菜单完成定位后聚焦；异步禁用拦截交互并保留已获得焦点。
- 原分段单选结构改由 SegmentedControl 唯一拥有，删除业务拼装的 ccc-tabs/ccc-tab。固件/软件风扇、风扇选择、功耗通道、独显状态及键盘类型均接入受控契约，硬件草稿和写入流程不变。
- scripts/ui-contract.mjs 是校验契约唯一来源；TypeScript/PostCSS/选择器 AST 检查原生入口、全局私有类及子部件、业务别名和内联视觉覆盖。允许业务布局、Surface 内容排版、状态查询与图形数据；负例验证禁止路径，check:boundaries 随构建执行。
- 所有权同步 DESIGN.md，回归入口同步 README.md。仅新增显式开发依赖并同步锁文件，不添加运行时 UI 框架、Provider、兼容适配或产品测试开关。
- WebView2 验收使用 tools/ui-webview2 独立 Wry 宿主及模拟 IPC；固定测试端口，数据/日志/截图隔离在 target/ui-check，编译在 target/ui-webview2-build。宿主只支持验收用的四档缩放，不加载生产 HAL、控制器、真实配置或硬件；WebView2 Loader 从锁定依赖解析并打包。

## 影响与验证

- npm run build 通过（TypeScript、三页边界检查、Vite）；cargo check --workspace --offline 通过，仅有现有增量缓存硬链接降级复制警告。未运行完整测试套件。
- npm run check:controls：25 项边界与定位测试全部通过，覆盖原生及旧入口拒绝、Surface/子部件/别名/内联外观、选择器函数、布局与状态查询，以及上下展开、窄视口和多档缩放。
- npm run check:ui：12 项 Edge 浏览器回归全部通过，覆盖实际三页和全部下拉/分段接线、草稿保留、文件输入引用、异步字体保存焦点、主题与 360/700/1280px、普通/搜索菜单键盘、禁用/空值/异步选项/错误、共享原语及两步确认、缩放/翻转/滚动跟随/离屏关闭。
- npm run check:ui:native：12 项 WebView2 检查全部通过，运行时 157.0.4341.46；验证普通/搜索菜单、四档 CSS 根缩放、四档与生产相同的 WebView2 ZoomFactor、实际控制台草稿与设置接线。无 JavaScript 异常，截图已检查；证据在 target/ui-check/native-1794560037863/。全部 IPC 为隔离模拟，不代表真实硬件验收。
- 原生宿主初次缺失 GNU 动态 Loader、跨平台离线依赖解析及测试夹具遮挡问题均已修复并复验；失败诊断留在各次独立目录。无需浏览器下载，浏览器回归使用本机 Edge。
- 相关 140 个源文件的 500 行文件/80 行函数限制检查通过，源码无旧 Combobox/原生选择/ccc-tabs 路径、冲突标记或差异空白错误。笔记生命周期转换后运行 verify-notes。
- 不提交、推送、切换分支或部署；保留其他任务的硬件、插画、参数说明与偏好改动。

## 备选方案

- 保留原生 Select 与单独 Combobox，或适配旧 children/event：形成两份事实和过渡路径，拒绝。
- 只补正则或靠人工保持一致：无法可靠识别子部件和别名，拒绝。
- 引入运行时 UI/定位框架或让生产应用加载测试开关：当前需求不需要，拒绝。
