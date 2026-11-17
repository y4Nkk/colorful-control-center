# Colorful Control Center 设计契约

> 本文是 UI / 交互设计事实的唯一所有者。硬件通道架构见 `docs/design.md`；
> 每条规则都是「决定 + 归属 + 禁止」的契约句式。非平凡设计变更先写决策笔记。

## 产品目标

Colorful Control Center 是七彩虹游戏本（蓝天模具）的本地硬件控制台。界面的首要任务是：写入前让用户理解将发生的变更，写入后确认实际状态，失败时给出可操作的恢复路径。硬件写入安全优先于一切功能展示。

设计优先级：

1. 传感器事实与执行状态优先于装饰；硬件概览使用品牌标识与自绘部件插画增强识别，装饰不遮挡读数、不冒充实物型号。
2. 每项硬件写入都有唯一执行入口、预览、确认与恢复语义（安全设计详见 `docs/design.md` §4）。
3. 同一概念在不同页面使用同一控件、术语和视觉层级。
4. 状态同时提供简短标签及可访问语义，不只依赖颜色和动画；完整规则与技术诊断可展开查看。
5. 操作以用户减负为目的，后台识别协议与能力；预设采用“选卡片预览→确认应用”，避免重复武装同一确认。高级参数按需展开，保留真实变更、错误与恢复入口。

## 视觉语言

- 安静、清晰的本地工具界面：灰白底色建立骨架，间距、排版和圆角表达层级，蓝色表达操作和选中，状态色表达真实业务结果（安全绿 / 警告橙 / 错误红）。
- 不使用纯黑背景、大面积渐变、装饰性光晕或霓虹游戏风元素。
- 预设模式使用内置生图生成、本地保存的语义场景图：绿叶/能量、羽毛/静水、播放/音符、闪电/速度分别对应省电、安静、娱乐、性能，静音增强复用安静图。图中不出现笔记本或其他设备模型，不包含文字、实时指标或交互；图片保留原色，外框、文案及原生单选控件表达实际状态与主题。原自绘笔记本 SVG 和专属材质令牌移除；图片来源与完整提示词归 public/mode-artwork/SOURCES.md。
- 寄存器地址、路径、JSON、数值读数使用等宽字体（`.ccc-mono`）。
- 界面符号由 `src/components/Icon.tsx` 唯一接入开源 Lucide React，以显式命名导入生成本地 SVG，使用 currentColor；导航24px、折叠及状态20px、窗口与按钮16px，选中导航提高描边权重。页面到导航图标的语义映射只在 navigation.ts 清单中定义，模块标题与操作按语义显式接入；不复制库路径，不使用远程图标或字符代替状态图标。品牌图标、图表与专用硬件示意画继续由各自所有者负责。
- 应用标识采用透明背景的五叶液态玻璃风扇，叶间负空间保持透明，不绘制圆形底盘、外圆环或底座；左下蓝经紫粉至右上琥珀。玻璃厚度、折射高光与缓慢内部流彩仅用于品牌资产，不扩展到数据与操作界面。`src-tauri/icons/icon.svg` 唯一拥有图形与材料，`BrandLogo` 读取同源并隔离实例 ID，响应系统及应用减少动效；禁止模块复制路径或自绘第二版。`scripts/generate-icons.mjs` 唯一生成 favicon 与桌面图标，构建检查受管产物一致性，不手动维护副本。
- 硬件插画允许局部金属明暗与 PCB/金手指材质色，颜色仍由 tokens.css 唯一拥有；品牌图片保留原色、比例与独立留白。官方标识从 public/brands/ 本地加载，来源由同目录 SOURCES.md 记录；不在运行时请求第三方图片。
- 软件内模块以优质SVG、已有生成场景图与关键数值建立识别；主视图少文字，长说明和完整参数归Collapsible，必要风险与错误就地保留。装饰复用领域插画所有者，保持模块边框与间距；不画无实际分母的比例，不以未来目标冒充当前实读。
- 配置页先显示名称、实际状态、目标控件和操作；删除重复用途、步骤和实现原理介绍，不将其改写成另一段常驻提示。参数帮助置于操作之后，默认折叠；阻止执行的原因、实际变更及必要影响仍就地显示。预设采用小图选择卡，性能/键盘开关并列，CPU/GPU调节宽屏左右排布；不使用CSS统一隐藏caption或截断诊断。

## Windows 安装向导

- `installer/` 唯一拥有原生 WPF 安装外壳：欢迎、安装位置、正在安装、完成四步；安装位置展示当前用户范围、目录与替换已有版本语义，安装前明确确认。步骤以序号、文字状态与颜色共同表达，安装执行中禁止重复提交及关闭。
- 安装向导采用Apple风格的浅白底、充足留白与大图：左侧55%宽度展示 `installer/assets/installer-artwork.png` 的透明抽象玻璃曲面图，小型品牌置顶、版本置底；右侧只保留短标题、必要说明和操作。移除双风扇装饰、功能介绍、标语、重复步骤状态和完成页说明；日志折叠，错误仍直接可见。装饰不表达硬件事实，提示词归同目录SOURCES.md。长路径换行、小工作区滚动，Tab、Enter、Escape与系统缩放可用。
- 左栏使用Canvas浅灰、右栏使用Surface白色，均从tokens.css生成画刷。四步进度只显示8px状态点，移除序号、勾号及可见步骤标签；当前点强调、完成点减淡，完整步骤名称和状态由原生Label的辅助技术名称提供。
- 白色面板的左上/左下圆角以28px向灰栏叠入，内容保留原对齐，图形让出叠入范围；形状随窗口裁切。`InstallerWindowMotion.cs`拥有步骤淡入/方向位移与状态点过渡，进度刷新不重播，不延迟安装状态提交；系统关闭动画、高对比度或零时长直接显示。圆角、位移和时长从tokens.css生成。
- 位置页首次安装显示“选择安装位置”，更新显示原目录和“打开安装目录”；更新不隐式迁移文件或自启路径。安装后摘要以实际NSIS内嵌主程序为唯一基准，诊断记录实际差异，不把摘要不一致归咎于权限。
- `installer/Theme.xaml` 拥有原生控件样式，构建时从 `src/styles/tokens.css` 生成所需画刷，禁止手工复制颜色事实。网页控件仍由既有组件拥有，不在主应用新增安装页。
- NSIS唯一拥有安装副作用、WebView2准备、快捷方式及卸载注册；外壳内嵌引擎并核对成功退出和主程序摘要。提取阶段显示真实字节比例，安装阶段不定进度，不以时间或文件大小猜测总体百分比。失败保留日志及重试入口，完成后明确选择启动或结束。
- 进程占用也由NSIS按目标文件唯一检查，WPF不维护第二份进程名判断。异常与日志路径使用结构化状态，显示文案不承担解析协议，诊断格式由InstallerFailure.cs拥有。Build-InstallerShell是唯一封装实现；安装包校验后再替换最终文件，记录提交失败回滚原包。
- `scripts/build-windows-installer.ps1` 是 Windows 用户安装包唯一构建入口，输出 `target/release/bundle/installer/`；NSIS产物是内部引擎。不引入MSI、更新器、多语言或自动启动硬件控制。

## 主题

- 浅色 / 深色双主题由 `src/styles/tokens.css` 唯一拥有：按 `documentElement[data-theme]` 映射语义变量，页面不另造配色，不得在 tokens.css 之外定义颜色。
- 默认跟随系统；软件偏好由后端 `preferences/` 严格校验并原子写入 `app-preferences.json`，`src/lib/appearance.ts` 是前端主题、字体、动效应用唯一入口。删除旧主题 localStorage 读写，不双读、双写或保留兼容路径。
- 设置页的主题预览通过 `data-preview-theme` 局部作用域复用同一浅深色板；预览不改根主题，不另存颜色，不覆盖减少动画设置。

## 图形化应用设置

- `SettingsWorkspace` 只拥有软件自身设置，按外观、窗口与托盘、运行日志、渲染性能、关于分组；沿用 agent-switchboard 的图形单选卡、预览、系统字体搜索选择与 Switch。机器配置按硬件分类页面接入，启动性能档案归自动策略页，设置页不保留重复入口。
- 外观拥有主题、系统字体、90/100/110/125% 界面缩放和跟随系统/减少动效；颜色、排版与时长仍由全局令牌拥有，减少动效同时尊重系统与用户选择，预览不绕过该偏好。
- 窗口与托盘拥有关闭到托盘/退出、始终置顶、开机自启、启动时隐藏、控制台/上次访问启动页和全局唤起快捷键。关闭按钮文案跟随已确认策略；启动页只在启动时解析一次，最近访问页只保存一级页面，不保存硬件草稿或确认态。快捷键使用物理键标准编码，录入/切页/失焦及时解除录入抑制，注册冲突保留原绑定。
- 开机启动状态由 `src/app/useAutostart.ts` 唯一拥有：未知／读取中禁用写入，读取失败可重试，写入中锁定重复请求，写入后回读再展示实际状态；失败保留已确认值，回读失败将状态标记为未知。
- 运行日志级别即时控制后续日志写入，静默不删除旧日志；渲染硬件加速在下一次创建 WebView 时应用，界面显示是否需要重启并提供真实重启操作。关于信息取实际后端版本，失败就地重试，不在设置主流程展示硬编码硬件通道说明。
- 软件偏好保存先严格重读文件并与已确认快照比较，再应用原生窗口/快捷键并原子持久化；进程启动后文件损坏或外部改写时，在原生执行前拒绝覆盖并要求重新读取。执行或持久化失败尝试恢复原桌面偏好并展示原始错误与恢复错误。只有文件不存在才使用默认值，损坏、缺字段或未知字段拒绝读取且保留原文件；设置页可重试，不显示虚构的已保存值。
- 登录自启由 `startup/login` 拥有，固定当前用户任务是唯一运行入口；设置页 `AutostartSetting` 预览关闭/普通/管理员方式，保存与恢复首次前态均二次确认。配置时进行一次系统授权；后台 GPU/LAN 仅沿用现有管理员令牌，不临时弹 UAC。旧本软件 Run 项只捕获为转换前态，确认任务后移除，不作为回退。偏好 JSON 不保存重复开关，`login-startup.json` 只保存首次前态与本次已提交目标。

## 字体、间距与控件

- 正文使用 `Noto Sans SC`（系统栈，无 webfont 依赖）；代码和路径使用系统等宽字体。
- 页面标题、模块标题、标签、正文、说明各自遵守 tokens.css 字阶，不以元素默认样式决定层级。
- 间距来自全局令牌；模块间使用容器 `gap`，不叠加随意外边距。
- 圆角由全局尺度拥有：12（基础控件）、16（中型卡面）、20（导航项）、24（内部信息组、弹层）、28px（工作区、主要面板）及全圆角。全圆角是形状语义（状态胶囊、圆点），不是尺度档；不新增档位。
- 标准交互控件高度 40px，紧凑行内控件 32px，由 tokens.css 令牌拥有。
- 界面语言为中文单语（决策笔记 2026-11-12-foundation-contract）：直接写中文文案，文案不做深层字符串拼接，保持未来可机械抽取。

## 页面结构

`components/control/AirplaneSettings.tsx`在键盘与输入页唯一挂载系统飞行模式卡片；左侧显示系统报告，右侧保留当前／开启／关闭，不增加网卡配置。选择只预览，确认后同步原厂固件与专属设备，独立确认恢复首次状态。未知状态、来源变化及失败保留后端诊断；离散开关使用语义图标与左右模块，不绘制比例环。`AirplaneArtwork`拥有纯装饰折纸SVG，复用语义颜色并随主题呈现，不表达无线设备状态。原厂面板维护通过热键范围接入，说明启动 NVSR 修复与事件 OLED 唤醒的不同触发，不增加重复开关。

主导航按任务分类：控制台只保留实时监控；风扇散热、性能调校、显示与显卡、灯光效果、键盘与输入、电池与节能、自动策略分别由Cooling/Performance/Graphics/Lighting/Input/Battery/AutomationWorkspace唯一挂载领域编辑器。性能档案独立，软件设置固定在导航底部。页面标识、标题与语义图标由 `src/app/navigation.ts` 唯一拥有，AppWorkspace保活已访问页面；不在控制台保留重复编辑入口。

预设模式由 `components/control/OemPowerMode.tsx` 拥有，唯一挂载在性能调校页顶部；省电、安静、娱乐、性能四张小图单选卡显示名称与状态，固件不支持时禁用并标明，静音增强仅在能力声明时补充。`useOemPowerMode.ts` 拥有读取、草稿及执行状态；读取失败清除可执行快照，重试成功前禁止写入。当前使用与待应用分开标明，卡片选择不写硬件；当前到目标及原厂联动预览、确认应用和恢复初始复用同一执行链。个人参数仍由性能档案保存，档案预览包含模式名称。

`components/control/PowerProfilePolicy.tsx`唯一挂载供电与唤醒档案卡片；左侧显示供电、实际启用状态、已保存规则与最近自动应用，右侧编辑插电/电池/唤醒规则，使用Surface模块边框及左右间距，窄窗口纵排。选择只改草稿，复用ProfilePreview完整参数和ConfirmButton两步确认；启用不立即应用当前状态，临时暂停、重新读取并暂停、恢复初始规则并暂停分别表达软件行为。显示试用仍由唯一ProfileApplicationHost确认。`usePowerProfilePolicy.ts`拥有读取、规则草稿与异步选择，`useAutomaticProfileReport.ts`只读取应用Controller的最近自动结果，不生成第二份执行报告。

`components/control/SupplyModeSettings.tsx`在自动策略页唯一挂载原厂供电模式；左侧当前事实与最近自动结果，右侧六种条件的只读联动卡片，宽屏左右、窄屏纵排。启用/继续明确先应用当前启动规则，均绑定完整预览并两步确认；暂停与初始恢复分开，授权等待期间仍能请求暂停。恢复展示当前到初始的模式/选项偏好、风扇、系统计划、固件节能、独显和频率，原始风扇目标可展开；关闭独显期间的未知读数与开启后取得的恢复基线不混同。离散对象不绘制比例，表面与边框由Surface拥有。

- 应用外框是无原生标题栏的两行两列 grid：导航跨左侧两行、集成标题栏右上（64px）、工作区右下。侧栏在 224px 图标文字模式与 64px 图标模式之间切换，手动折叠偏好由 AppShell 唯一持有，以 `localStorage` 的 `ccc-sidebar-collapsed` 布尔字符串持久保存；首次使用默认展开，启动首帧同步恢复，手动切换立即保存，彻底退出后重开仍恢复。保存失败用全局通知反馈并保留已确认状态，不依赖退出事件保存。导航项高 40px、圆角 20px，选中态用 `--ccc-nav-selected` 底色 + 加粗正文色，`aria-current="page"` 来自同一页面事实。设置入口固定底部，折叠后仍保留中文可访问名称及悬停提示。
- 顶栏整合产品标识、当前页面标题、全局信息位与应用内窗口按钮。顶栏和侧栏顶部的非交互区可拖动窗口，双击拖动区切换最大化；交互按钮不属于拖动区。最小化、最大化／还原、关闭由 WindowControls 唯一实现，最大化图标跟随真实窗口状态，关闭按已保存的软件偏好隐藏到托盘或退出；普通浏览器预览不显示无效的窗口按钮。
- 侧栏与顶栏共享连续的高斯模糊材质：Tauri 主窗口透明并启用 acrylic 背景，body 透明，AppShell 统一绘制半透明底色与背景模糊，参数由 tokens.css 拥有；不对文字、图标或页面内容使用 filter: blur，不在两个区域各叠一层材质。工作区保持不透明，保证数据与表单可读。
- 工作区 28px 圆角、完整 overflow 裁切，由全局 `ScrollArea` 提供唯一滚动面；固定外框内的 viewport 原生滚动，页面内容只保留工作区内留白，滚动不进入顶栏或导航。
- 两级骨架契约：一级工作区骨架由顶栏标题行拥有（标题 + 全局信息位）；二级编辑器骨架（固定头 + 滚动模块区 + 固定动作栏）在第一个二级页出现时引入 `EditorFrame` 原语，两级互不混用。
- 页面挂载是 keep-alive：`AppWorkspace` 用 visited 集合 + `hidden` 切换，保住草稿/滚动/展开状态；`active` prop 控制轮询等副作用只在可见页运行。

## 全局能力与组件所有权

窗口宽度不超过 700px 时使用图标导航，为编辑区域保留空间；自动折叠不写入持久偏好，回到宽窗口恢复用户原来的手动折叠偏好。

「全局定义 → 模块接入」唯一模式：全局能力 = 模块级单例 store + 命令式 API，app 层唯一挂载 Host；模块位置只 import 调用。禁止 Provider 树与第二实现。

| 所有者 | 契约 |
|---|---|
| `src/components/control/DisplayModeVisual.tsx` | 显示器模式SVG，取实际/明确目标分辨率、方向和频率；当前与目标单独标明，未知显示空状态，不模拟正在扫描的物理画面，档案复用同一绘图 |
| `src/components/control/KeyboardBacklightVisual.tsx` | 白光键盘的当前保存值与亮度草稿SVG；几何键帽为示意，灯光随明确开关/亮度变化，未知不绘制点亮，不伪装真实灯效回读 |
| `src/components/control/PowerSourceVisual.tsx` | 插电/电池/备用/未知来源的语义供电SVG，瓦数只使用后端明确报告，不把PD档位换算为功率；自动规则复用来源图形但明确规则目标 |
| `src/styles/configuration-visuals.css` | 配置领域的绘图尺寸、图形/数值/控件布局和响应；色值仍归tokens.css，表面边框归Surface，不改全局控件外观 |
| `src/components/control/HotkeyEventPreviews.tsx` | 热键已选/已保存事件及恢复事实的唯一组合；草稿范围与操作入口在前，事件事实随后显示，错误及恢复不折叠隐藏，复用各事实组件 |
| `src/styles/tokens.css` | 视觉令牌唯一所有者（颜色/圆角/间距/字阶/动效），`check:boundaries` 强制 |
| `src/components/BrandLogo.tsx` / `src-tauri/icons/icon.svg` / `scripts/generate-icons.mjs` | 应用品牌标识：SVG 唯一绘图源，组件隔离实例并接入减少动效；生成入口同步 favicon 与桌面图标，检查模式拒绝过期/缺失资产。业务只传布局与 SVG 定位，不自建材料 |
| `src/styles/base.css` | reset + 全局硬约束 + 领域样式聚合入口（`@import`） |
| `scripts/ui-contract.mjs` / `check-ui-boundaries.mjs` | 控件边界校验唯一契约：TypeScript/PostCSS AST 检查原生入口、全局私有类及子部件、业务类别名和内联视觉覆盖；`check:boundaries` 随构建执行，负例由 `check:controls` 验证 |
| `src/api/client.ts` | IPC 唯一边界：唯一 `invoke()` 封装 + 领域 re-export 聚合；`check:boundaries` 强制 |
| `src/components/use-toast.ts` | 全局通知唯一实现：命令式 `toast()` 任意模块可调，`useToast()` 供渲染方订阅 |
| `src/components/Toaster.tsx` | 通知唯一渲染 Host，app 层挂载一次；模块禁止自建浮层/弹窗。原厂低温电池提醒由同一Host展示，状态由后端唯一持有，显示温度与电量并提供确认关闭，插电或停止对应会话后关闭；不受短时OSD开关影响 |
| `src/components/Button.tsx` / `src/styles/button.css` | 按钮唯一原语：主次操作、紧凑操作、图标、导航、选择、曲线拖动点与颜色键帽的外观及禁用/危险态集中定义；可传语义icon由全局Icon绘制，业务只传行为、内容和布局，保留可访问名称 |
| `src/components/Input.tsx` / `src/styles/field.css` | 文本、数字、颜色与隐藏文件输入唯一入口，原生属性及 ref 透传；40/32px 高、12px 圆角、边框、禁用、无效和焦点样式全局统一 |
| `src/components/Select.tsx` / `SelectMenu.tsx` / `use-select.ts` / `use-select-position.ts` / `src/styles/select.css` | 下拉框唯一实现：普通与搜索选择共用受控 value/options/onChange(value) 契约、字段样式和自绘菜单；菜单挂 body，统一边缘避让、上下定位、缩放换算、限高滚动、键盘、焦点和关闭行为；禁止原生 select/option、旧 children/event 契约及平行搜索实现 |
| `src/components/Checkbox.tsx` / `src/components/Radio.tsx` / `src/styles/choice.css` | 复选框及单选输入唯一原语；Checkbox 用于草稿布尔选择，Radio 拥有普通与卡片/分段覆盖输入，保留原生表单语义；即时异步开关仍用 Switch |
| `src/components/SegmentedControl.tsx` / `src/styles/choice.css` | 分段单选唯一结构：受控 value/options/onChange，自动生成唯一单选组，统一容器、选项、焦点及禁用状态；业务只提供选项、标签、行为和布局类，不拼容器/label/input |
| `src/components/Slider.tsx` / `src/styles/slider.css` | 原生 range 唯一原语：轨道、滑块、焦点和禁用外观集中拥有；ScaleSlider 只组合业务读数、单位及当前值标记 |
| `src/components/Surface.tsx` / `src/styles/surface.css` | 面板、卡片、信息组、工作区与内嵌区的表面/边框/圆角唯一原语；业务只保留布局，选择卡片的悬停、选中、焦点及禁用状态同源 |
| `src/components/Switch.tsx` | 开关唯一原语：原生 checkbox 提供 switch 语义与键盘行为，CSS 自绘轨道、滑块和焦点；异步保存用 aria-disabled 加输入拦截阻止重复操作，同时保留键盘焦点 |
| `src/components/ScrollArea.tsx` | 滚动容器唯一原语：固定外框 + 可聚焦原生 viewport；通过viewportRef向app交接定位能力，AppShell唯一保存每页会话滚动位置；侧栏与工作区统一接入 |
| `src/components/Collapsible.tsx` / `src/styles/collapsible.css` | 内容展开/收起唯一原语：原生 details/summary 拥有开合与键盘语义，共享箭头、焦点、紧凑点击区与内容间距；收起保留子内容挂载，跟随页面 keep-alive 保留开合状态；模块只传标题与内容，不直接拼 details/summary 或自建折叠样式。侧栏宽度切换仍归 AppShell/SideNav 所有 |
| `src/styles/scroll-area.css` | 全局滚动条唯一样式：透明轨道、细滑块，空闲隐藏，滚动、滑块悬停或拖动时显示；尺寸、浅深颜色与空闲延时由 tokens.css 拥有 |
| `src/app/useScrollActivityMarker.ts` | 全局滚动活动唯一所有者：App 根调用一次，被动捕获原生滚动，逐面标记并在空闲 900ms 后清除；卸载清理监听、计时器与标记 |
| `src/components/settings/` | 软件偏好分组、PreferenceCards、系统 FontPicker、ShortcutSetting 与关于；settings.css 只拥有布局和业务内容，控件及容器外观归对应全局原语；settings-previews.css 拥有示意画语法，预览颜色复用 tokens.css |
| `src/app/useAppPreferences.ts` | 已确认偏好快照、读取/保存/重启互斥、错误和原生异步诊断唯一所有者；App 根唯一调用，工作区只组合 |
| `src/api/preferences.ts` / `src-tauri/src/preferences/` | 软件偏好 IPC 与严格文件契约、桌面执行/补偿、系统字体、快捷键及启动窗口配置；日志持久化使用 Tauri 官方日志插件 |
| `src/app/useAutostart.ts` | 开机启动读取、保存、回读、并发锁与错误状态唯一所有者 |
| `ConfirmButton`（components/ConfirmButton.tsx） | 硬件写入的两步确认唯一实现；主应用用 primary、恢复用 secondary，确认态统一错误红并保持对比度，3 秒未确认自动复位 |
| `FactList`（components/FactList.tsx） | 键值事实唯一原语（`dashboard.css` 拥有语法）：rows用于事实行，tiles以标签/值上下排列的无行线网格用于语义恢复分组；不自建平行事实行 |
| `src/components/control/HardwareParameters.tsx` / `ParameterHelp.tsx` / `parameterDescriptions.ts` | 模块内硬件参数与术语说明：FactList 的 compact 布局列出真实参数，Collapsible 提供可键盘展开的解释；字段未知时明确标注，规格、实时值、偏移与限制值分开命名，离线仍可阅读说明 |
| `CapacityOverview.tsx` 的内存频率事实 | 系统容量与原厂共享报告分别命名；报告频率和参考频率直读 MHz，未知不补零，参考值不作为硬上限或比例分母，说明共享报告延迟与 DDR 有效速率的区别 |
| `api/hardware.ts` | 硬件快照类型与订阅唯一封装（`hardware-snapshot` 事件 1Hz） |
| `src-tauri/src/hal/overview.rs` / `src/app/useSystemOverview.ts` | 系统只读概览契约：CPUID 身份、物理内存、固定磁盘卷；可见页的轮询、重试与再进入共用串行读取，旧请求结束前不发新请求，过时代次不回写，读取中禁用重试；失败保持未知与诊断，不走 OEM/硬件写入通道 |
| `src/components/control/HardwareArtwork.tsx` / `ProcessorArtwork.tsx` / `GraphicsCardArtwork.tsx` / `ModuleArtwork.tsx` / `CapacityOverview.tsx` | 品牌标识、按实物参考重绘的四类部件与容量卡：CPU 只传单一品牌，由 ProcessorArtwork 派生顶盖与文字；非 CPU 图形不接收品牌参数。GPU 使用桌面双风扇整卡，内存使用鳍片马甲，存储使用 M.2 标签/缺口/安装孔；装饰使用 aria-hidden，不把参考商品型号当作本机事实，容量有真实分母才绘制使用量 |
| `src/components/control/` | 温度趋势、HardwareReadings/MetricReadout 语义读数、可拖动曲线、数值滑杆与短变更预览；按分类工作区组合，拖动只改草稿 |
| `src/components/control/RpmCurveEditor.tsx` / `rpmCurve.ts` | 十级 RPM 阈值编辑与草稿约束；复用 CurvePointInputs 的数值控件，纵轴是该风扇最大 RPM 百分比，按阈值画阶梯 |
| `src/components/control/SoftwareFanPanel.tsx` / `SoftwareCurveEditor.tsx` | 软件曲线草稿、逐点拖动/键盘/数值、添加删除、回差和斜坡参数；同一风扇区选择固件或软件执行，预览确认后才接管，退出恢复自动 |
| `src/styles/monitoring.css` | 温度历史、CPU/GPU模块、左右布局和真实容量/占用环形比例的唯一样式；档案功耗、电压和频率使用带单位的数值事实，不以人为上限绘制比例 |
| `src/components/performance/GpuTuning.tsx` | GPU 偏移区唯一实现，NVAPI 类型化读写、回读和恢复；与控制台组合 |
| `src/components/control/KeyboardLighting.tsx` / `KeyboardEffectEditor.tsx` / `useKeyboardLighting.ts` | 键盘九种模式、三区颜色/开关、亮度草稿与首次写入前恢复；模式与颜色变化撤销待确认态，诊断折叠展示 |
| `src/components/control/TopCoverEditor.tsx` | 独立顶盖当前/草稿的亮度和颜色；亮度0关闭，复用灯光策略确认及恢复，区别于多区顶盖 |
| `src/components/control/LightingOptions.tsx` / `useLightingOptions.ts` | 闲置关灯、开机覆盖及能力声明的Logo/普通灯条/独立顶盖；模块内左右布局、独立草稿预览、两步确认和初始恢复。重新读取不丢草稿，读取失败撤销可执行快照；恢复只编辑所属字段 |
| `src/components/control/LightBarSettings.tsx` / `LightBarEditor.tsx` / `lightbar-values.ts` / `useLightBarSettings.ts` | X170/X58独立灯条的严格型号草稿与编辑器；X170八模式/RGB/开关，X58六模式/七色/原厂亮度与速度。共用自研示意、当前/目标左右卡片、预览确认及初始恢复，无能力隐藏，未知保存值不冒充默认，型号变化显式重载草稿 |
| `src/components/control/MultiZoneLighting.tsx` / `MultiZoneEditor.tsx` / `useMultiZoneLighting.ts` | type23四区键盘及按能力提供的灯条/顶盖颜色；当前与草稿左右模块、预览确认和共享初始恢复，读取不初始化保存值 |
| `src/components/control/MultiZoneColors.tsx` / `src/components/profiles/ProfileMultiZonePreview.tsx` | 控制台与档案共用多区颜色示意，按实际能力显示灯区；模式/速度/亮度按单位展示，不冒充物理回读 |
| `src/components/control/PerkeyLighting.tsx` / `PerkeyPatternEditor.tsx` / `usePerkeyLighting.ts` | 逐键静态与动态灯效、MCU能力限制、速度/方向/颜色草稿、变更预览和确认恢复；按键响应状态及失败重试不冒充灯效回读 |
| `src/components/control/PerkeyKeycaps.tsx` / `src/lib/perkey-layout.ts` / `perkey-layout-data.json` | 原厂布局事实生成的键帽与LED对应关系；大键同时选择全部对应灯位，未知布局明确使用灯位矩阵。外观由全局Button的keycap变体拥有，不复制原厂UI样式 |
| `src/components/control/KeyboardSettings.tsx` | Fn锁、Win锁与Fn/Win交换草稿；按固件实时状态开放、逐项预览、统一确认及初始恢复，共享字段由keyboard域保留 |
| `src/components/control/TouchpadSettings.tsx` / `useTouchpadSettings.ts` | 触摸板系统状态与保存偏好分开展示，左侧当前与恢复目标、右侧草稿及实际差异；预览确认、失败补偿和首次恢复，设备变化撤销旧执行资格，窄屏纵向模块布局 |
| `src/components/profiles/ProfileTouchpadPreview.tsx` | 档案触摸板设备集合、系统目标及保存偏好，左右模块预览；不以保存值冒充系统状态 |
| `src/components/control/MicrophoneSettings.tsx` / `useMicrophoneSettings.ts` | 活动麦克风当前状态与全部静音/取消静音草稿，左右模块、预览确认及本会话首次混合状态恢复；不录音，不在读取时应用偏好 |
| `src/components/profiles/ProfileMicrophonePreview.tsx` | 档案与启动摘要的设备集合、逐端点静音及保存偏好只读预览；复用统一档案确认 |
| `src/components/keymap/` / `src/styles/keymap.css` | 键盘/鼠标映射、宏序列/延时/文本草稿、显式录制、变更预览和恢复；当前保存来源与草稿左右展示，记录缓冲不自动应用，失焦及离页结束录制 |
| `src/components/Textarea.tsx` | 多行输入唯一原语，和Input共用field.css的边框、字体、焦点与禁用状态；业务只提供内容、行数、标签和布局 |
| `src/components/control/ScreenBrightnessFacts.tsx` | 原厂屏幕亮度策略的百分比环实际报告、计划AC/DC保存值、完整触发目标与逐计划初始恢复预览；复用事实卡和全局确认，恢复与热键软件规则分开 |
| `src/components/control/GpuTurboFacts.tsx` | 原厂 GPU Turbo 的设备当前状态与各模式/供电事件目标左右分组；锁频与偏移只显示实际值，不伪造比例，初始恢复展示当前到目标并独立确认 |
| `src/components/control/BatteryThresholdFacts.tsx` | 原厂低电量阈值的当前计划与事件目标左右分组，目标复用百分比环；各计划恢复值单独分组，模块边框和间距使用Surface，窄屏回流。独立授权、确认与恢复不与充电保护阈值混用 |
| `src/components/control/BatteryLowBrightnessFacts.tsx` | 低电量亮度以插电与电池的真实计划值环分别展示，当前与后端实际设置目标左右分组，附自然结束目标；恢复区显示首次接管前的两个独立值与确认动作，窄屏回流并保留模块边框，明确事件可能使屏幕变暗，已有缓存时不固定绘制0%目标 |
| `src/components/control/BatteryAlertFacts.tsx` | 原厂电池音频事件的循环状态、PC Beep离散静音及首次恢复，左右Surface分组、全局图标和少量必要说明；绝对亮度与低电量共用亮度事实卡和恢复入口，不把播放状态或开关画成百分比 |
| `src/components/control/BatteryLowActionFacts.tsx` | 低电量与严重电量的系统动作以离散文字展示，不绘制比例；左右模块分别显示当前计划与事件规则，明确睡眠、休眠是一过性动作及原厂不额外请求关机的分支 |
| `src/components/control/PdEventFacts.tsx` | 原厂PD事件当前设备与触发目标左右分组，按固件编号语义显示等级；复用平台初始基线，确认后停止监听并恢复。离散等级不伪造百分比，模块内不用分隔线代替语义间距 |
| `src/components/control/PanelHotkeyFacts.tsx` | 原厂屏幕按键当前适用行为与本软件交互会话左右分组；独立授权前展示OLED空白屏保或一次性输入唤醒规则，确认结束只清理本软件会话并停止监听，不推断物理屏幕状态或伪造反向硬件命令 |
| `src/components/control/HotkeySettings.tsx` / `useHotkeySettings.ts` | 原厂热键范围与屏显草稿、规则预览（分区、颜色循环、亮度循环/反馈、只读状态通知及摄像头报告同步分别独立授权）、接管确认、临时停止及初始软件策略恢复；亮度事件说明RGB主动循环与白光报告同步的区别，产品文案不暴露原始型号编号。后台操作复用领域事务，硬件初始恢复保留在对应模块。状态通知标为原厂报告，最后结果与是否显示桌面提示分开；键盘灯快捷入口由应用根切换控制台，工作区挂载后定位键盘区域 |
| `src/components/OsdHost.tsx` / `src/styles/osd.css` / `src-tauri/src/osd/` | 桌面屏显唯一Host：独立透明、置顶、无焦点、穿透窗口；展示已提交操作结果、已授权原厂状态报告或明确预览样例。序列号隔离旧消息与计时任务，窗口只允许读取屏显状态；不挂载主应用或硬件轮询 |
| `src/components/control/PlatformPower.tsx` / `usePlatformPower.ts` | PD档位按原厂候选编辑，卡片内左侧当前事实、右侧目标与操作，窄屏回流；预览确认和原始等级恢复，Deep S5只读状态。无能力隐藏，读取失败撤销执行快照；不把PD等级冒充CPU瓦数 |
| `src/components/control/UsbPowerPolicy.tsx` / `useUsbPowerPolicy.ts` | USB选择性暂停：固件建议、三个标准计划的插电/电池现值、左右草稿和完整差异预览；确认后仅写USB字段，初始恢复不切回旧计划，无能力隐藏，读取失败撤销执行快照 |
| `src/components/control/LanPolicy.tsx` / `useLanPolicy.ts` / `lan-values.ts` | 网络驱动策略：具体网卡、节能及原厂唤醒补丁草稿；后端权威差异预览、独立系统授权、逐字段持久恢复。当前与目标左右分组，未知类型保留，不将驱动设置回读称为物理唤醒已生效 |
| `src/components/control/FanDust.tsx` / `DustScheduleEditor.tsx` / `useFanDust.ts` | 除尘左侧展示固件运行与一次性操作，右侧星期/整点/唤醒计划草稿及确认恢复；无虚构停止或百分比。轮询保留草稿，读取失败撤销执行快照，未知原计划与清除本次触发明确分开 |
| `src/components/control/DisplaySettings.tsx` | 每台活动显示器的频率及主屏方向预览；方向按当前与目标左右分组并保留模块边框，试用可确认保留、取消或15秒自动恢复，首次模式可单独恢复；计时恢复由后端拥有 |
| `src/components/control/AutomaticRefreshSettings.tsx` / `useAutomaticRefresh.ts` | 自动刷新率独立模块：当前主屏与供电、原厂候选和启用后的规则左右分组，启用前展示本次可能的目标并确认；停用恢复初始屏幕与原厂保存字段，离散频率不绘制比例。监听失败、外部修改和恢复资格直接展示 |
| `src/components/control/SupplyModeSettings.tsx` / `SupplyModeScenarios.tsx` / `SupplyModeCurrent.tsx` / `SupplyModeRecovery.tsx` / `useSupplyMode.ts` / `src/styles/supply-mode.css` | 原厂自动模式的六条件、当前事实、最近结果及完整初始恢复；独立启用/继续/暂停/恢复、失效读请求、模块边框和左右布局，通知结果由后端拥有 |
| `src/components/control/gpu-values.ts` / `power-plan-values.ts` | GPU原始锁/偏移及目标、Windows计划/链路节能名称的共用展示；未知保留未知，不建立另一份执行规则 |
| `src/components/AutomaticDisplayTrialHost.tsx` | 自动刷新率试用确认的唯一全局Host，应用根挂载；任何页面或隐藏到托盘后唤起时均可看到当前至目标、剩余时间和保留／恢复。保留前由后端复核供电和模式，超时由独立监护恢复；手动与档案试用不在此Host重复确认 |
| `src/components/control/GpuPowerSettings.tsx` | 原厂独显控制节点与自动关断策略预览，单次授权、失败补偿及混合初态恢复；关断暂停GPU轮询和调校，阻止影响活动输出 |
| `src/components/control/GraphicsModeSettings.tsx` | 固件报告的显卡模式、待重启选择和重启后确认分开展示；发送、恢复与重启分别确认，记录跨应用重启保留 |
| `src/components/control/BatterySettings.tsx` / `useBatterySettings.ts` | 电池电量与容量比例、充电阈值草稿及确认恢复；与节能、校准共用字段时互斥，刷新事实保留未提交草稿。读取失败撤销Profile和执行快照，读取成功清除读取错误，动作错误单独保留。候选来自固件，电源未知时保留空值 |
| `src/components/control/BatteryOverview.tsx` / `BatteryArtwork.tsx` / `src/styles/battery-visuals.css` | 电池语义SVG、系统电量及供电/充电报告、OEM满充/设计容量与循环次数；概览左右模块，窄屏纵排。填充只取实际百分比，未知无填充，0%保留0；充电标记只取明确充电报告，插电不推断充电。容量比例保留原始超过100%的数值，循环次数不画百分比。校准复用同一电量图形，读取任务实际电量，不用阶段序号冒充进度 |
| `src/components/control/EnergyPolicySettings.tsx` / `EnergyPolicyEditor.tsx` / `useEnergyPolicy.ts` | 节能计划的星期、两段整点时段、自动/手动阈值、时间条预览、独立确认、停止和初始恢复；保存意图与固件充电回读分开展示，不将时段显示为当前正在执行 |
| `src/components/control/BatteryCalibrationSettings.tsx` / `useBatteryCalibration.ts` | 电池校准的五阶段、真实状态与耗时、接管及恢复目标预览、独立开始/停止/恢复确认；页面不拥有任务生命周期，不把阶段序号或电量伪装为总体耗时进度 |
| `src/components/control/StartupProfilePicker.tsx`、`useStartupProfile.ts` | 控制台启动档案选择、共享ProfilePreview及当前组合模式联动、保存确认与最近执行结果；hook拥有唯一读取/保存状态，保存状态未知时仍可明确重新选择或取消；确认绑定所选文件，联动预览标明启动时重新检查，不保存为下次执行授权；软件开机启动开关仍由设置页拥有 |
| `src/components/control/OemModeEffects.tsx`、`energy-values.ts` | 控制台、档案与供电条件共用的原厂模式联动预览；插电偏好、固件节能、风扇、偏移、Turbo/DTT、ASPM与系统计划按语义展示，节能值与硬件管理共用唯一格式化函数 |
| `src/components/control/OemModeRestore.tsx` | 单项模式与性能选项共用的初始恢复预览和确认；填满可用宽度，按模式与性能、风扇、Windows计划分组，以FactList的tiles布局减少内框和行线；完整固定目标、插电偏好所有权及可展开风扇原值绑定初始快照摘要 |
| `src/components/control/OemModeGpuEffects.tsx` / `OemModeGpuRestore.tsx` / `GpuModeFacts.tsx` | 模式GPU当前/目标、独显动作时序、驱动范围和首次恢复来源；宽屏左右、窄屏纵排，Surface保留模块边框和间距。GpuModeFacts唯一展示原始锁频、偏移及未读取原因，供电与明确选择共用；离散模式不画比例，关闭独显时不伪造频率 |
| `src/components/control/ModeCycleFacts.tsx` | 热键循环的完整模式候选、插电/电池频率及独显阶段预览；左侧实读、右侧按模式分组，原始频率与驱动范围复用GpuModeFacts，恢复仍在预设模式模块进行 |
| `src/components/control/GpuPowerVisual.tsx` / `GpuNodeStages.tsx` | GPU精细插画与短电源状态、已预览节点阶段图；复用HardwareArtwork显卡SVG，离散状态不绘制比例，阶段仅由真实预览请求产生，诊断与完整参数可展开 |
| `src/components/profiles/ProfileModeAuthorization.tsx` | 后台档案完整模式许可预览：复用GPU供电目标表和语义图标，展示未来AC/DC频率、允许的自动关断条件与驱动延后状态；长规则可展开，不把当前raw值作为长期许可 |
| `src/components/control/HotkeyCurrent.tsx` / `hotkey-actions.ts` | 热键运行状态与暂停、规则名称和说明的唯一事实源；只展示已选范围、当前接管或仍有恢复记录的详细能力，恢复入口不因取消草稿选择而消失 |
| `src/components/control/OemPerformanceOptions.tsx` / `useOemPerformanceOptions.ts` | Turbo风扇与DTT独立草稿、支持能力/驱动/模式约束和两步确认；Turbo变化预览自动风扇联动，恢复复用原厂模式初始快照 |
| `src/components/control/ModeArtwork.tsx` / `src/styles/mode-presets.css` | 模式到本地生成图片的唯一映射与卡片布局；装饰图片使用空 alt、aria-hidden，不接受拖动或指针，表面和单选交互复用 Surface / Radio |
| `src/app/ControlWorkspace.tsx` | 各调节域刷新代次唯一所有者；热键及档案应用id/阶段修订触发事实回读，倒计时不重复刷新，不重新挂载页面。已编辑草稿保留，未编辑草稿采用新事实；独显操作后刷新GPU调校 |
| `src/app/useProfileReads.ts` / `useProfiles.ts` | 档案列表、文件与模式目标的请求代次；写入使旧读取失效，托盘请求在写入结束后消费一次 |
| `src/components/profiles/ProfileGpuPowerPreview.tsx` | 档案独显节点与自动关断策略；左右分组、独立开关事实和目标，身份归折叠诊断，明确软件风扇退出及开启后驱动/温度复核，不用比例图表示布尔状态 |
| `src/components/profiles/ProfilePreview.tsx` / `ProfileDevicePreview.tsx` | 档案风扇与参数预览，含已读取的原厂联动目标；设备卡复用LightBarPreview/型号摘要与除尘星期语义，以左右分组卡展示。离线允许查看导出，缺少联动预览时禁止手动应用 |
| `src/components/profiles/ProfileGraphicsModePreview.tsx` | 档案MUX目标、当前启动报告及已知待重启选择左右分组；明确请求接受与实际切换的边界，不提供自动重启或伪比例 |
| `src/components/profiles/ProfileKeyboardPreview.tsx` / `control/keyboard-values.ts` / `control/lighting-option-values.ts` | 档案键盘功能、RGB/白光及外围策略卡；复用编辑页字段与摘要，左右布局、独立开关/亮度和灯区色块示意，不复用禁用编辑器或伪造比例 |
| `src/components/profiles/ProfilePerkeyPreview.tsx` | 档案逐键灯效只读键帽图；复用原厂布局及LED索引，未知布局显示矩阵并说明；动态效果展示配置色彩和方向，不伪造实际运行画面，按键响应仅说明应用后的启动意图 |
| `src/components/profiles/ProfileKeymapPreview.tsx` / `src/lib/keymap.ts` | 档案映射/宏的设备范围、启用与程序监听意图；复用按键名称和完整动作摘要，左右卡片、设备图形和可展开的全部目标，不执行录制或宏、不把保存意图当作过滤器回读 |
| `src/components/profiles/ProfilePlatformPreview.tsx` / `control/platform-values.ts` | 档案PD与USB目标；共用等级和标准计划名称，左右分组、离散等级选择标记与逐计划AC/DC事实，不把档位或开关伪造成连续比例 |
| `src/components/profiles/ProfileBatteryPreview.tsx` / `control/EnergyTimeline.tsx` | 档案充电及节能目标；左右模块、明确阈值环与原始关闭状态、星期和真实24小时时段。时间条由控制台编辑与档案共用，预览只展示配置，不触发校准或假定物理充电效果 |
| `src/components/profiles/ProfilePerformancePreview.tsx` / `control/OemPerformanceOptions.tsx` | 档案Turbo/DTT保存偏好与实际模式语义；共享字段名称与状态文字，左右卡片、独立风扇联动说明；组合模式预览由preview_profile_mode提供，不从普通模式目标猜测覆盖值 |
| `src/components/profiles/ProfileLanPreview.tsx` / `control/lan-values.ts` | 档案逐网卡驱动目标、节能及唤醒语义；左右模块卡片，缺失目标明确表示恢复未设置，原始类型和身份归折叠诊断；复用LAN字段名称及状态文字，不把驱动字段显示为比例 |
| `src/components/profiles/ProfileDisplayPreview.tsx` | 档案每台显示器的身份/几何与当前至目标刷新率；左右模块卡片，离线标明当前未知，不用比例表达频率；启动预览说明仍需试用保留 |
| `src/app/profile-application.ts` / `src/components/ProfileApplicationHost.tsx` | 档案应用结果与全局显示确认的唯一前端状态；App唯一挂载，整组15秒试用明确保留后继续其余设置，取消/超时/失败分别展示，不把待确认显示为全部成功 |
| `ConfirmButton` 禁用态 | 所有异步写入都支持 disabled/busy，确认文本跟随当前草稿；重复写入禁用 |
| `src/components/PagePlaceholder.tsx` | 占位工作面唯一原语；真实模块接入时整体替换内容 |
| `src/app/navigation.ts` | 页面注册唯一清单；页面标识唯一来源 |
| `src/app/AppShell.tsx` | 外框唯一所有者；页面不拼装标题/导航/骨架 |
| `src/app/SideNav.tsx` | 侧栏唯一实现：顶部折叠入口、图标导航与底部设置；页面清单仍来自 navigation.ts |
| `src/components/Icon.tsx` | Lucide React唯一边界：显式导入、语义名称及统一尺寸；默认装饰隐藏于辅助技术，独立状态可传label提供名称；icons.css拥有颜色继承与导航选中描边，图标不接收点击与焦点 |
| `src/app/WindowControls.tsx` | 集成标题栏窗口按钮唯一实现，监听真实最大化状态，失败使用全局通知 |
| `src/api/window.ts` | 桌面窗口类型化 API 唯一封装；经 client.ts 聚合，窗口操作由 commands/window.rs 拥有 |
| `src/app/AppWorkspace.tsx` | 工作区挂载唯一所有者（keep-alive 接线） |
| `src/lib/appearance.ts` | 已确认软件外观应用与系统主题监听唯一入口；真实桌面缩放由原生 WebView 设置，浏览器验收只镜像该比例 |
| `src/lib/startup-page.ts` | 最近一级页面的本地布局状态与启动解析；与侧栏布局状态独立，均不保存机器配置 |
| `src-tauri/src/command_registry.rs` | 全部 IPC 命令唯一注册清单 |

新增全局原语必须先在本表登记，再写实现；模块只能在本表原语上组合业务，不复制基础控件结构、不创造平行组件。

## 图形控制台

- 同一网格行的模块共享顶部与底部边界：主调节区和设备区使用拉伸对齐，由该行最高内容决定高度；右侧调节组首尾分布，设备模块主要操作行靠底对齐。长文案、错误与展开说明参与自然布局，不用固定高度、裁切或空占位填平。单列时恢复各模块自然高度。
- CPU/GPU/内存/存储卡片列出可读取的规格和当前参数；CPU 的标称频率不冒充实时频率，GPU 当前功耗上限不冒充出厂最大功耗，磁盘卷不冒充物理硬盘型号。参数事实使用全局 FactList 的 compact 布局，说明使用全局 Collapsible，主视图不展开长篇解释。
- 参数名统一中文：持续功耗上限、短时功耗上限、核心/核显电压偏移、核心/显存频率偏移；控件、预览、档案、启动摘要和用户可见错误保持一致。CPU/GPU 部件简称、标准单位与内部协议标识保留；风扇模式名称由 api/fans.ts 的 FAN_MODES 唯一拥有。

- 控制台只拥有监控概览；设备编辑器分布到注册的领域工作区，每个编辑器只有一个挂载点。领域工作区沿用控制模块样式，active由对应页面透传；app层唯一拥有历史订阅、跨域修订和逐页滚动，草稿随工作区保活。
- 键盘灯使用九种灯效选择、可点选的三区预览、独立颜色/开关和亮度滑杆；草稿先预览再确认。未知类型保留诊断和重新读取，不提供手动猜测协议的入口；不支持的型号禁止写入。
- 独立顶盖由能力声明后显示，复用外围灯选项事务；左侧当前亮度与颜色示意，右侧亮度和颜色草稿。亮度0关闭并保留色值，卡片有独立边框与间距，窄屏纵向排列；与多区顶盖区明确区分，档案及启动预览保持同一含义。
- 灯光策略与外围灯区独立分组：闲置和开机、Logo和灯条使用成对模块卡片与左右内容布局；仅显示本次支持的编辑项和有变化的预览行，窄屏纵向排列。恢复目标明确列出，键盘模式恢复不得覆盖这些选项。
- 温度由 TrendChart 按 CPU/GPU 上下分区显示采样柱状图：同一真实时间轴与 0–110°C 比例，每柱对应一次采样，最新柱加深、95°C 起标红；缺失值和停采时段留空，仅保留 95°C 参考线与 0°C 基线，不聚合、不插值。CPU/GPU 读数各有独立边框和间距的模块卡片，内部左侧标题与 RPM，右侧环形占空比/利用率/显存读数；CPU利用率使用有采样时间的Windows处理器效用，与风扇占空比分开。空值或未知容量保留空环，RPM 不虚构比例；不绘制重复微型折线。窄屏保持左右分区，比例环自动换行。
- 风扇编辑图按能力表达温度—占空比两点控制关系或十级目标 RPM 阈值，无密集网格、投影虚线或图框；点位标注仅在悬停、聚焦、拖动时出现，曲线目标在图外标为预估，不冒充实时占空比。拖动与数字输入共用草稿，当前曲线仅在差异时作为参考。
- 已核对型号可选择软件方式：每把 2–12 点可增删/拖动/键盘/数值编辑，95°C/100% 安全端点固定；首点之前保持恒定的真实控制关系也画入曲线。回差和斜坡独立设置，显示 CPU/GPU 较高温度作为输入。切换方式只改变预览，确认应用才切换控制；退出和异常后保留草稿，不自动重新接管。
- 十级表的每个温度阈值可调且严格递增，目标转速单调不降、末级 100%（末级温度可低于 100°C）。读取保留原厂停转值；明确编辑或选择自定义时，新手动目标最低 20%，全部差异可见后再确认。档案按每把风扇分图，无重复网格。
- 控制区用表面色与留白分组，移除逐卡描边。滑杆仅保留操作轨道，边界值在交互时显示；主应用与次级恢复有明确层级。
- 图表以实际采样时间定位，缺失值保留断点，过期数据明确标记。没有硬件数据时不生成示例读数。
- 文案只保留数值、单位、短标签、状态和动作；实现细节与原始读数放折叠诊断，禁止在主流程放通道说明长段落。
- 草稿跨页面保留，拖动不写硬件；应用前可见变更差异，确认后执行，执行中禁用重复操作；恢复调用后端记录的原始状态。
- 功耗区在同组滑杆中切换「立即应用／保存设置」，分别呈现 MSR 回读与持久化值；两者的草稿独立保留，保存结果不表述为实时生效。PL1/PL2 联动保持有序。
- 一次 CPU 确认提交全部变更字段，失败恢复整组；档案预览按「立即应用 · CPU」「保存设置 · CPU」「立即应用 · GPU」分组，用语义量表显示数值，电压编码换算为 mV。手动确认绑定完整预览，同名内容变化也重置确认；拒绝旧预览时清除旧执行结果，恢复失败有明确提示。
- 拖动风扇曲线自动选择自定义模式；模式、曲线与加速偏移先预览，确认后按序执行，首个失败即停止。托盘选择档案只打开预览。
- OEM 电源模式预览同时列出原厂风扇、偏移、PCIe 节能与 Windows 电源计划目标。确认绑定整份联动目标，原厂保存策略变化后必须重新读取确认；Quiet Plus 仅在能力页声明时出现。

## 状态与反馈

- 操作进行中 / 错误在所属工作面就地反馈，`CommandError` 文本直接呈现。
- 全局通知（toast）用于跨工作面的异步结果与提醒；普通队列上限 2 条，错误置顶。原厂低温电池提醒单独保持至用户确认或原厂关闭条件，复用同一Host并展示明确原因。
- 持续性全局状态挂在顶栏右侧全局信息位（`AppShell` 的 `globalInfo`），这是唯一常驻全局反馈面。
- 反馈不得只靠颜色：状态文字与语义色同时出现。

## 禁止项

1. 不建平行组件：写新 UI 前先查「组件所有权」表；同一原语全应用唯一实现。
2. 不在 tokens.css 之外定义颜色、圆角、间距、时长常量（`check:boundaries` 强制）。
3. 组件不得直接 `invoke()` 或 import `@tauri-apps`——只经 `api/` 类型化封装（`check:boundaries` 强制）。
4. 不自建路由、页面切换或第二导航；页面变更走 `navigation.ts` 注册表 + `AppWorkspace` 接线。
5. UI 不直写硬件：任何 EC / MSR 写入必须经 HAL 契约（预览 → 确认 → 执行 → 可恢复），安全语义见 `docs/design.md` §4。
6. 不引入路由库 / 状态管理库 / UI 框架 / CSS 框架，除非决策笔记先行并修订本文。
