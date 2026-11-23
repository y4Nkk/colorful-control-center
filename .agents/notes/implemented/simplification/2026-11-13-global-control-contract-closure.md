# Agent Note: 全局控件契约与选择菜单完整收敛

Status: implemented

## 问题

2026-11-13-global-controls-and-surfaces 完成多数入口迁移后，普通 Select 仍输出原生 select/option，搜索菜单另用 Combobox，展开面不统一且缺少边缘定位；样式边界未覆盖 Surface、子部件及业务别名。用户明确要求完整、无兼容、无过渡、无技术债实现全局化组件并统一接入。本笔记取代已归档的旧笔记；折叠组件原有笔记继续有效。

## 决定

- 2026-11-23 全局控件禁用时显示禁止光标，包括下拉触发器及菜单项、滑杆、页签和选择控件。进度光标由实际执行动作的按钮或分段选项拥有。确认按钮用自身提交状态标记执行进度，模块忙碌仍可锁定确认入口；分段单选用待完成值定位正在处理的选项。禁用菜单项保留指针命中，以显示禁止光标，选择逻辑继续跳过禁用项。
- 全局原语统一覆盖 Button、Input、Select、Checkbox、Radio、SegmentedControl、Slider、Surface、Switch、ScrollArea 与 Collapsible。所有业务入口直接迁移；边框、圆角、尺寸、状态和焦点只由全局样式及 tokens.css 拥有，业务保留布局、内容和行为。
- Select 只有受控 value/options/onChange(value) 当前契约，普通与搜索选择共用一个实现。删除 Combobox、原生 select/option、children/event 契约、ccc-combobox 及旧原生选择样式；字体读取仍由 FontPicker 拥有。
- SelectMenu 挂所属对话框或 body，复用 Button 和 Input，列表框拥有菜单内滚动。use-select 统一键盘、搜索、禁用、关闭和焦点，use-select-position 及纯函数 select-layout 统一上下展开、边缘避让、限高、滚动跟随与根缩放换算。搜索框在菜单完成定位后聚焦；异步禁用拦截交互并保留已获得焦点。
- 原分段单选结构改由 SegmentedControl 唯一拥有，删除业务拼装的 ccc-tabs/ccc-tab。固件/软件风扇、风扇选择、功耗通道、独显状态及键盘类型均接入受控契约，硬件草稿和写入流程不变。
- scripts/ui-contract.mjs 是校验契约唯一来源；TypeScript/PostCSS/选择器 AST 检查原生入口、全局私有类及子部件、业务别名和内联视觉覆盖。允许业务布局、Surface 内容排版、状态查询与图形数据；负例验证禁止路径，check:boundaries 随构建执行。
- 所有权同步 DESIGN.md，回归入口同步 README.md。仅新增显式开发依赖并同步锁文件，不添加运行时 UI 框架、Provider、兼容适配或产品测试开关。
- WebView2 验收使用 tools/ui-webview2 独立 Wry 宿主及模拟 IPC；固定测试端口，数据/日志/截图隔离在 target/ui-check，编译在 target/ui-webview2-build。宿主只支持验收用的四档缩放，不加载生产 HAL、控制器、真实配置或硬件；WebView2 Loader 从锁定依赖解析并打包。
- 2026-11-19 增加全局 `ColorPicker` 和 `ColorSwatch`。可编辑色块以应用令牌绘制，展开后通过色域、色相、RGB 或 HEX 编辑相同的 RGB 草稿。复用 Button、Input 和 Slider，以 Popover 和 CSS 锚定位处理浮层和视口。全部自定义颜色入口直接切换到该契约，移除通用 Input 的原生颜色类型、旧业务样式及按钮背景特例。固定色表仍由设备声明。
- 2026-11-19 按 Agent Switchboard 的全局 Button、Slider、Select 和浅深主题调整公共控件。主次按钮、图标操作和禁用反馈共用 Button；原生 range 负责数值与输入，胶囊轨道、填充、刻度和滑块共用同一几何，色相轨道用实际色谱。Select 的展开菜单以触发器为最小宽度、按内容适配，统一右侧勾选、导航高亮和视口避让。主题沿用 tokens.css，保留设备草稿、确认和搜索语义。
- 2026-11-19 详情与确认共用 Agent Switchboard 的弹窗尺寸、标题和滚动结构。`Dialog` 保有原生模态隔离，展示时聚焦标题，关闭后由原生对话框恢复焦点。详情支持图标、Escape 和遮罩关闭，遮罩手势优先收起内层菜单。编辑和确认保留明确关闭与完成动作，忙碌时拦截用户关闭。`FactList` 的行布局按最宽标签对齐值列，窄窗限制标签列并让两列换行。弹窗、正文块和行使用留白安排层级。交互参考 [Apple Sheets](https://developer.apple.com/design/human-interface-guidelines/sheets)、[WAI-ARIA Modal Dialog](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/) 与 [React Aria Modal](https://react-aria.adobe.com/Modal)，键值布局对照 [Ant Design Descriptions](https://ant.design/components/descriptions/) 的无边框样式。

- 2026-11-23 全局 Tabs 统一拥有内容页签、面板关联、键盘操作及样式。电源区域、风扇、CPU 调校方式、键鼠映射、键盘灯区和档案参数视图同时迁移。方向键、Home 与 End 仅移动焦点，Enter 或空格选择当前焦点项。电源区域保活已访问面板，其他页签按选择挂载内容。独立设置继续使用其选择或操作原语。

## 影响与验证

- 2026-11-23 页签迁移通过前端构建、Rust 工作区离线检查、33 项控件检查和 73 个聚焦 Edge 用例。首轮通过 71 项。键鼠映射保留选中项的修复与通知入口断言更新后，两个失败用例复测通过。浅深主题下核对 360px 和 1440px 的页签布局，方向键、Home、End、Enter、空格、Tab 焦点及面板关联均经浏览器验证。硬件交互使用隔离模拟 IPC。

- 2026-11-19 共享详情改造通过前端构建、Rust 离线检查、32 项控件检查和 25 个隔离浏览器场景。确认标准、宽幅、简短确认三档宽度，360px 窗口的正文独立滚动与固定操作区，以及深浅遮罩、Tab 循环、焦点归还、嵌套弹窗和菜单优先关闭。最终遮罩与短窗编辑器已复验，截图位于 `target/ui-check/details-dialog-20261119/`。浏览器使用模拟 IPC，原生窗口实测待验。

- 2026-11-19 全局控件视觉调整通过 32 项边界与定位检查，以及 27 个隔离浏览器场景。浅深主题分别与参考 CSS 比对标准按钮、选择器、行高亮、菜单边框与阴影、滑块轨道与滑块。模拟 IPC 验证 360–1440px 页面布局、搜索和长列表、对话框内选择、滑块端点、应用确认和草稿。安装器画刷读取消费的九个颜色角色。截图位于 `target/ui-check/agent-switchboard-controls-20261119/`。原生窗口与硬件待验。

- 2026-11-19 全局色块接入后，前端构建、Rust 离线检查和 32 项控件边界与定位检查通过。28 个聚焦浏览器场景经首轮和复验通过，覆盖 RGB/HEX 校验、拖选、窄窗定位、色环、逐键扫描双色、多区、外围灯与档案只读色块。独立 WebView2 宿主在 100% 和 125% 原生缩放下验证定位、数值联动与焦点恢复。灯光使用模拟 IPC。
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
