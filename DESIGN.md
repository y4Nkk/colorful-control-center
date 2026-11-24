# Colorful Control Center 设计契约

> 本文是 UI / 交互设计事实的唯一所有者。硬件通道架构见 `docs/design.md`；
> 每条规则都是「决定 + 归属 + 禁止」的契约句式。非平凡设计变更先写决策笔记。

## 产品目标

Colorful Control Center 是七彩虹游戏本（蓝天模具）的本地配置工具。写入前展示将发生的变更，写入后确认实际状态，失败时给出可操作的恢复路径。

设计优先级：

1. 传感器事实与执行状态优先于装饰；硬件概览使用品牌标识与自绘部件插画增强识别，装饰不遮挡读数、不冒充实物型号。
2. 每项硬件写入都有唯一执行入口、预览、确认与恢复语义（安全设计详见 `docs/design.md` §4）。
3. 同一概念在不同页面使用同一控件、术语和视觉层级。
4. 关键状态使用简短文字和可访问语义，完整规则与技术诊断按需展开。
5. 后台识别协议与能力。输入或选择模式后，页面展示真实差异和联动影响，点击一次应用即提交。高级参数按需展开，错误和恢复目标保持可见可查。

## 视觉语言

配色与排版探索可参考 getdesign 收录的 [Apple 设计分析](docs/design-references/apple/DESIGN.md)（[MIT 许可](docs/design-references/apple/LICENSE)）。

全局按钮、选择器、滑块、弹窗、键值行与浅深配色采用 Agent Switchboard 的视觉规则，参考源码由 y4Nkk 以 MIT 许可发布。[许可](docs/design-references/agent-switchboard/LICENSE)

- 安静、清晰的本地工具界面：灰白底色建立骨架，间距、排版和圆角表达层级，蓝色表达操作和选中，状态色表达真实业务结果（安全绿 / 警告橙 / 错误红）。
- 不使用纯黑背景、大面积渐变、装饰性光晕或霓虹游戏风元素。
- 性能与散热的预设模式卡片在中央展示 SVG 插画，当前档名与分段单选依次置于图形下方。省电用玉绿双叶舒展和掠过叶脉的亮线，安静与静音增强用冷蓝月弧升落、扩展水纹和收放星芒，娱乐用紫色双轨反向绕行，性能用暖橙三层风刃和长尾迹向右上推进。四套循环各有节奏，所有动态装饰限制在图形画幅内。当前档名和可用选项的文字采用同档渐变，浅深主题分别使用相应的文字色阶。选择目标时在按钮与状态行反馈进度，主图、档名和选中项由设备回读决定。回读在不同图形间切换时，上一档按原色淡出，当前档沿其图形方向入场。首次确认已知模式时直接开始循环，未知或停滞画面静止，离开活动页停播。系统或应用启用减少动效时直接显示静态插画。短窗压缩图形高度，窄窗允许五档选择器换行。
- 寄存器地址、路径、JSON、数值读数使用等宽字体（`.ccc-mono`）。
- 界面符号由 `src/components/Icon.tsx` 以显式命名导入接入图标库并生成继承 currentColor 的本地 SVG。导航采用 24px Phosphor 线框／实心配对，其他界面符号采用 Lucide，折叠及状态 20px，窗口与按钮 16px。页面到导航图标的语义映射只在 navigation.ts 清单中定义，模块标题与操作按语义显式接入；不复制库路径，不使用远程图标或字符代替状态图标。品牌图标、图表与专用硬件示意画继续由各自所有者负责。
- 应用标识采用透明背景的液态玻璃双翼气流，上翼为电光蓝至紫红，下翼为洋红至亮橙。两翼围出透明通道，玻璃厚度、折射亮边和缓慢流彩裁切在翼面内。品牌材质不扩展到数据与操作界面。`src-tauri/icons/icon.svg` 唯一拥有图形与材料，`BrandLogo` 读取同源并隔离实例 ID，响应系统及应用减少动效，强制色彩呈现双翼剪影；禁止模块复制路径或自绘第二版。`scripts/generate-icons.mjs` 唯一生成 favicon 与桌面图标，构建检查受管产物一致性，不手动维护副本。
- 硬件插画允许局部金属明暗与 PCB/金手指材质色，颜色仍由 tokens.css 唯一拥有；品牌图片保留原色、比例与独立留白。官方标识从 public/brands/ 本地加载，来源由同目录 SOURCES.md 记录；不在运行时请求第三方图片。
- 概览顶部使用宽幅只读模式卡，与性能与散热页共用 SVG 插画和渐变档名。档名居中置于插画下方。短窗压缩图形，换档动效跟随采样。供电预览和热键候选使用静态模式小图标，静音增强复用安静月牙。
- 用实际状态、可操作控件和有明确含义的图形建立模块层级。同一模块的完整参数、诊断与说明由共享详情按钮打开，在弹窗内按内容分区。仅图标按钮提供可访问名称及悬停提示。电源与档案页面不设信息图标入口，诊断与说明随入口一并移除，失败与阻止原因直接留在卡内。相关模块同排对齐，窄窗口纵排。必要风险和阻止操作的短原因留在控件旁。
- 开关、选项和动作按钮表达本身的状态，邻近文字用于显示额外事实，例如部分设备状态、草稿和实读差异、执行进度及操作限制。参数差异与联动目标复用键值事实布局。
- 禁用的控件显示禁止光标，包括下拉触发器、菜单项、滑杆和页签。只有正在处理本次操作的控件显示进度光标，同组被暂时锁定的其他控件继续显示禁止光标。
- 采用 [Apple 布局指南](https://developer.apple.com/design/human-interface-guidelines/layout) 的逐层披露、[PowerToys 设置](https://microsoft.github.io/PowerToys/core/settings/project-overview/) 的任务分组和 [Home Assistant 卡片](https://www.home-assistant.io/dashboards/tile/) 的当前值与详情分离。品牌和硬件插画沿用本应用的视觉语言。
- 配置页先显示名称、实际状态和目标控件。参数帮助和完整诊断由按钮打开弹窗。就地显示阻止执行的原因，在页面预览中确认实际变更与必要影响。预设采用带图选择卡，性能和键盘开关并列。CPU/GPU 调节各占通栏，按参数分组并随可用宽度回流。

## Windows 安装向导

- `installer/` 唯一拥有原生 WPF 安装外壳：欢迎、安装位置、正在安装、完成四步；安装位置展示当前用户范围、目录与替换已有版本语义，安装前明确确认。步骤以序号、文字状态与颜色共同表达，安装执行中禁止重复提交及关闭。
- 安装向导采用Apple风格的浅白底、充足留白与大图：左侧55%宽度展示 `installer/assets/installer-artwork.png` 的透明抽象玻璃曲面图，小型品牌置顶、版本置底；右侧只保留短标题、必要说明和操作。移除双风扇装饰、功能介绍、标语、重复步骤状态和完成页说明；日志折叠，错误仍直接可见。装饰不表达硬件事实，提示词归同目录SOURCES.md。长路径换行、小工作区滚动，Tab、Enter、Escape与系统缩放可用。
- 左栏使用Canvas浅灰、右栏使用Surface白色，均从tokens.css生成画刷。四步进度只显示8px状态点，移除序号、勾号及可见步骤标签；当前点强调、完成点减淡，完整步骤名称和状态由原生Label的辅助技术名称提供。
- 白色面板的左上/左下圆角以28px向灰栏叠入，内容保留原对齐，图形让出叠入范围；形状随窗口裁切。`InstallerWindowMotion.cs`拥有步骤淡入/方向位移与状态点过渡，进度刷新不重播，不延迟安装状态提交；系统关闭动画、高对比度或零时长直接显示。圆角、位移和时长从tokens.css生成。
- 安装向导主窗口在透明边距中绘制浅阴影，内容区和标题拖动区按内侧边界布局。
- 位置页首次安装和旧地址不可用时允许选择最终目录。有效的旧安装固定原目录并提供“打开安装目录”。确认和重试均检查本地磁盘、目录创建权限和路径长度，文件写入交由 NSIS。窄窗口为操作区留出宽度，页脚按钮按可用宽度换行。安装后摘要以实际 NSIS 内嵌主程序为唯一基准，诊断记录实际差异。
- `installer/Theme.xaml` 拥有原生控件样式，构建时从 `src/styles/tokens.css` 生成所需画刷，禁止手工复制颜色事实。网页控件仍由既有组件拥有，不在主应用新增安装页。
- NSIS唯一拥有安装副作用、WebView2准备、快捷方式及卸载注册；外壳内嵌引擎并核对成功退出和主程序摘要。提取阶段显示真实字节比例，安装阶段不定进度，不以时间或文件大小猜测总体百分比。失败保留日志及重试入口，完成后明确选择启动或结束。
- NSIS 按目标文件检查进程占用，专用退出码区分运行占用与检查失败。向导在占用时弹出「关闭并安装」，取消返回安装位置。用户确认后，向导只请求安装目录里的控制台主进程正常退出，等待程序文件解除占用，再继续 NSIS 安装。关闭失败时提供托盘退出指引与重试入口。异常和日志路径使用结构化状态，诊断格式由 `InstallerFailure.cs` 拥有。`Build-InstallerShell` 校验后替换最终文件，记录提交失败时回滚原包。
- `scripts/build-windows-installer.ps1` 是 Windows 用户安装包唯一构建入口，输出 `target/release/bundle/installer/`。NSIS 产物是内部引擎，手动下载和应用内更新共用同一安装向导。

## 主题

- 浅色 / 深色双主题由 `src/styles/tokens.css` 唯一拥有：按 `documentElement[data-theme]` 映射语义变量，页面不另造配色，不得在 tokens.css 之外定义颜色。
- 默认跟随系统；软件偏好由后端 `preferences/` 严格校验并原子写入 `app-preferences.json`，`src/lib/appearance.ts` 是前端主题、字体、动效应用唯一入口。删除旧主题 localStorage 读写，不双读、双写或保留兼容路径。
- 设置页的主题预览通过 `data-preview-theme` 局部作用域复用同一浅深色板；预览不改根主题，不另存颜色，不覆盖减少动画设置。

## 图形化应用设置

- `SettingsWorkspace` 只拥有软件自身设置，按外观、窗口与托盘、运行日志、渲染性能、关于分组；沿用 agent-switchboard 的图形单选卡、预览、系统字体搜索选择与 Switch。机器配置按硬件分类页面接入，启动配置档案归电源页，设置页不保留重复入口。
- 外观拥有主题、系统字体、90/100/110/125% 界面缩放和跟随系统/减少动效；颜色、排版与时长仍由全局令牌拥有，减少动效同时尊重系统与用户选择，预览不绕过该偏好。
- 窗口与托盘拥有关闭到托盘/退出、始终置顶、登录自启、启动显示、启动时关闭原厂软件、概览/上次访问启动页和全局唤起快捷键。启动显示提供主界面和最小化到托盘两个选项，适用于登录自启和手动启动。默认显示主界面，桌面偏好读取或应用失败时也显示主界面。原厂软件退出默认开启，结果通过标题栏通知查看。关闭按钮文案跟随已确认策略；启动页只在启动时解析一次，最近访问页只保存一级页面，不保存硬件草稿或确认态。快捷键使用物理键标准编码，录入/切页/失焦及时解除录入抑制，注册冲突保留原绑定。
- Windows 托盘独立于主窗口处理输入。左键显示主窗口。右键在一级菜单列出固件支持的预设并以系统勾选标记当前档。预设项使用与主图同色的静态绿叶、蓝月、紫色环绕轨迹和橙色风刃，静音增强复用蓝月。菜单专用轮廓由 `src-tauri/icons/presets/` 拥有，`scripts/generate-icons.mjs` 使用预设配色令牌生成透明位图。「风扇散热」提供交还固件控制、确认全速和直达风扇设置。选择配置档案打开预览。悬停显示预设、风扇状态和有效的 CPU／GPU 温度。写入后回读再刷新菜单和主窗口，屏显反馈切换结果。收到退出请求后，托盘菜单显示「正在退出…」，正常收尾时移除图标。标题栏选择退出和渲染设置重启共用同一退出流程。
- 开机启动状态由 `src/app/useAutostart.ts` 唯一拥有：未知／读取中禁用写入，读取失败可重试，写入中锁定重复请求，写入后回读再展示实际状态；失败保留已确认值，回读失败将状态标记为未知。
- 运行日志级别即时控制后续日志写入，静默不删除旧日志；渲染硬件加速在下一次创建 WebView 时应用，界面显示是否需要重启并提供真实重启操作。关于信息取实际后端版本，失败就地重试。版本号旁的「更新日志」打开共享详情弹窗，默认读取内置 CHANGELOG.md 中的当前版本，可切换历史版本和明确标注的未发布内容。
- 「设置 → 关于」按需检查 GitHub 更新清单，显示现有版本、新版本、变更说明或检查结果。用户确认下载后显示实读进度，未知总量用不定进度。可在交接前取消下载，签名和版本核验通过后打开 Windows 安装向导，安装和关闭控制台的确认由向导负责。
- 软件偏好保存先严格重读文件并与已确认快照比较，再应用原生窗口/快捷键并原子持久化；进程启动后文件损坏或外部改写时，在原生执行前拒绝覆盖并要求重新读取。执行或持久化失败尝试恢复原桌面偏好并展示原始错误与恢复错误。只有文件不存在才使用默认值，损坏、缺字段或未知字段拒绝读取且保留原文件；设置页可重试，不显示虚构的已保存值。
- 登录自启由 `startup/login` 拥有，固定当前用户任务是唯一运行入口。设置页 `AutostartSetting` 预览关闭、普通和管理员方式，一次应用保存，首次前态单独确认恢复。配置时进行一次系统授权；后台 GPU/LAN 仅沿用现有管理员令牌，不临时弹 UAC。旧本软件 Run 项只捕获为转换前态，确认任务后移除，不作为回退。偏好 JSON 不保存重复开关，`login-startup.json` 只保存首次前态与本次已提交目标。

## 字体、间距与控件

- 正文使用 `Noto Sans SC`（系统栈，无 webfont 依赖）；代码和路径使用系统等宽字体。
- 页面标题、模块标题、标签、正文、说明各自遵守 tokens.css 字阶，不以元素默认样式决定层级。
- 模块标题行最小高度为紧凑控件的 32px。标题、图标、状态标签和操作在同一行垂直居中，空间不足时自然换行。状态标签按内容取宽，由所在行决定对齐。
- 间距来自全局令牌；模块间使用容器 `gap`，不叠加随意外边距。
- 模块、弹窗和菜单内的分组使用标题、留白、对齐或表面色。移除用于划分区域的横线。图表刻度、图例和闭合边框按各自语义绘制。
- 圆角由全局尺度拥有：12（基础控件、导航项、选择菜单）、16（中型卡面）、24（内部信息组、对话框）、28px（工作区、主要面板）及全圆角。全圆角用于状态胶囊和圆点。
- 标准交互控件高度 40px，紧凑行内控件 32px，由 tokens.css 令牌拥有。
- 界面语言为中文单语（决策笔记 2026-11-12-foundation-contract）：直接写中文文案，文案不做深层字符串拼接，保持未来可机械抽取。

## 页面结构

`DeviceConfigurationWorkspace` 中的选择和开关直接提交并回读。自动刷新率以扫描纹理面板显示主屏实读频率和供电模式，下方并列低刷、恢复两档。扫描纹理是动态背景装饰：光束自上而下扫过，三段光点沿扫描线行进；读数有效且页面活动时循环，读数未知、离页或系统与应用任一启用减少动效时显示静态纹理。节奏固定，不表示实际刷新时序，装饰留在自身画幅内且不遮挡读数。显卡模式用固件候选按钮切换，标签上方用无底色 SVG 示意各模式通路：选中项里位于通路上的 GPU 用实心挖空字形，未选中项用线框与浅底色，不在通路上的 GPU 淡出；通路以箭头指向屏幕，混合模式由独显汇入集显后输出，动态切换用点线通路汇入切换徽标。选中态与待重启提示由同一模式状态驱动。触摸板、麦克风与飞行模式并列，图形旁展示系统回读。

主导航依次提供概览、性能与散热、键盘与灯光、电源与档案、设备配置和设置。概览集中当前模式、CPU/GPU 频率与温度折线、内存与存储。性能与散热集中预设切换、性能选项、CPU／GPU 编辑器、风扇设置和除尘。风扇设置依次展示实况与风扇选择、曲线与点位、整机档位与偏移及恢复。说明与诊断按文字入口打开。电源与档案提供「电源设置」和「配置档案」，子区保活并记录独立滚动位置。配置档案无记录时展示保存和导入入口，有记录时显示配置组、规则引用及当前值与档案目标。托盘风扇入口定位风扇设置，托盘指定档案定位电源下的配置档案。设备配置集中屏幕、自动刷新、显卡、键盘功能、触摸板、麦克风和飞行模式。页面清单由 `src/app/navigation.ts` 拥有，`AppWorkspace` 保活已访问页面。

`PowerWorkspace` 集中电池与充电、节能与校准、档案应用及设备电源策略，使用设置页的内容宽度上限。电量摘要在前，充电设置接在下方；阈值随限制开关展开。启动档案、供电与唤醒开关及三条规则共用「档案应用」分组，标签靠左、控件靠右，子设置按行缩进。宽窄窗口共用排列及同页预览与恢复入口。参照 [Apple HIG 布局](https://developer.apple.com/design/human-interface-guidelines/layout) 与 [开关](https://developer.apple.com/design/human-interface-guidelines/toggles)。

`KeyboardWorkspace` 在同一滚动页面内呈现键盘灯光、按键映射与宏、原厂热键与屏显，以及灯光策略和外围灯区。键盘灯光占整行，映射和热键在下一行等宽并排，窄窗口改为纵排。各卡片的详情和重读统一放在标题右侧，映射与热键的编辑操作与主状态在同一行，空间不足时换行。灯光策略和外围灯区占整行。应用和恢复入口置于各自模块。

内存与存储分别占整行，磁盘卷随可用宽度排列。其他工作区以主任务为首个模块，同级项使用等高列或设置行。开关和离散选项直接提交并回读。曲线、连续参数、映射和档案在所属模块预览与应用，初始恢复也在同一模块。手动显示试用在屏幕模块确认，自动和档案试用沿用各自的确认区。实际待重启的目标在所属模块提示。

概览通过现有硬件采样的 `oemPowerMode` 展示当前模式。空值、未知编码和停滞读数显示未知与中性静态曲线。性能调校的预设入口由 `components/control/OemPowerMode.tsx` 拥有，点击或键盘激活分段选项即提交完整模式目标。固件不支持时禁用并标明，静音增强仅在能力声明时补充。遇到已核验的原厂 FnKey 时，使用共享对话框确认「关闭并切换」，显示目标模式、原厂热键影响和恢复方法。取消或离页放弃待确认的切换。`useOemPowerMode.ts` 拥有编辑器读取、确认、执行与回读状态，切换期间阻止重复提交，选中态以回读为准。读取失败清空模式状态并显示原因，重新读取成功后恢复可切换状态。个人参数由配置档案保存，档案预览包含模式名称。

`components/control/PowerProfilePolicy.tsx` 提供启用开关和三个触发条件，并提示已保存规则的暂停或等待状态。新规则在保存确认中经「查看档案参数」文字按钮预览完整档案，授权信息直接展示。已保存规则可直接启停，按下次供电或唤醒事件执行。失败与规则不可用的原因直接显示在卡内，出错时提供「重新读取并暂停」；供电来源与最近自动应用记录不在页面展示。显示试用由 ProfileApplicationHost 确认。

`components/control/SupplyModeSettings.tsx`显示当前模式和启用开关，供电六条件、频率实读和最近结果不在页面展示，阻止与失败原因直接显示在卡内。启用与继续在页面预览中显示启动联动，授予系统权限的等待期间仍能暂停。恢复预览显示实际初始值和可用状态。

- 应用外框是无原生标题栏的两行两列 grid：导航跨左侧两行、集成标题栏右上（64px）、工作区右下。侧栏展开 224px，收起 56px。列宽、标签淡入淡出和折叠图标翻转共用 250ms 时长和缓动；图标轴线固定，标签按展开宽度排版并随可用宽度裁切。手动折叠偏好由 AppShell 唯一持有，以 `localStorage` 的 `ccc-sidebar-collapsed` 布尔字符串持久保存。首次使用默认展开，启动首帧同步恢复，手动切换先保存再应用，保存失败用全局通知反馈并保留已确认状态。
- 导航、设置和折叠按钮共用 40px 高度、12px 圆角及独立悬停和按压叠层。当前项通过 `aria-current="page"` 显示中性底色、正文色加粗标签、蓝色实心图标和左缘 3×16px 蓝色指示条。切页时图标交叉淡化并轻弹，指示条用 320ms 拉伸至新位置，连续切换从当前帧接续。设置贴底并独立定位，主导航单独滚动。两种宽度共用同一组按钮和可访问名称，收起时额外显示指针和键盘提示。系统或应用启用减少动效时直接对齐目标状态。
- 顶栏左侧只显示品牌图标与软件名，右侧为全局信息位及应用内窗口按钮。当前页面名称用于侧栏与主工作区的可访问标签。顶栏和侧栏顶部的非交互区可拖动窗口，双击拖动区切换最大化；交互按钮不属于拖动区。置顶、最小化、最大化／还原、关闭由 WindowControls 唯一实现。图钉通过蓝色描边和按下状态显示已确认的置顶偏好，与设置页共用保存事务。最大化图标跟随真实窗口状态，关闭按已保存的软件偏好隐藏到托盘或退出。普通浏览器预览隐藏窗口按钮组。
- 侧栏与顶栏共享连续的高斯模糊材质：Tauri 主窗口透明并启用 acrylic 背景，body 透明，AppShell 统一绘制半透明底色与背景模糊，参数由 tokens.css 拥有；不对文字、图标或页面内容使用 filter: blur，不在两个区域各叠一层材质。工作区保持不透明，保证数据与表单可读。
- 工作区 28px 圆角、完整 overflow 裁切，由全局 `ScrollArea` 提供唯一滚动面；固定外框内的 viewport 原生滚动，页面内容只保留工作区内留白，滚动不进入顶栏或导航。
- 工作区右下角在当前页面滚离顶部后显示返回顶部。图标按钮避开滚动槽，页尾留出操作空间。激活后焦点回到工作区，系统或应用启用减少动效时立即返回顶部。
- 一级工作区由 AppShell 组合品牌顶栏、侧栏与滚动工作区。二级编辑器骨架（固定头 + 滚动模块区 + 固定动作栏）在第一个二级页出现时引入 `EditorFrame` 原语。
- 页面挂载是 keep-alive：`AppWorkspace` 用 visited 集合 + `hidden` 切换，保住草稿/滚动/展开状态；`active` prop 控制轮询等副作用只在可见页运行。

## 全局能力与组件所有权

窗口宽度不超过 700px 时使用图标导航，为编辑区域保留空间；自动折叠不写入持久偏好，回到宽窗口恢复用户原来的手动折叠偏好。

通知和原生事件等全局能力仍由 app 层唯一挂载。配置操作作用域由 AppWorkspace 按工作区创建，只收集所属页面已挂载模块的确认入口，不持有配置或硬件事实。

| 所有者 | 契约 |
|---|---|
| `src/components/Dialog.tsx` / `DetailsDialog.tsx` / `src/styles/dialog.css` | 共享原生模态对话框居中显示，宽度为标准 620px、宽幅 1000px 或简短确认 440px。标题与操作区固定在窗口边缘，正文独立滚动，各区以留白分组。打开时聚焦标题，关闭后恢复焦点，离页时关闭。普通详情可通过关闭按钮、Escape 或点按遮罩关闭，内层菜单优先响应。编辑及确认托管明确动作，忙碌期间锁定用户关闭。信息图标保留可访问名称和标题提示，内容打开后才挂载。弹窗正文两级文字层级：词条解释与上下文行用次级文字色，术语与事实值保持正文色；弹窗内错误与固件警示段落前置 error/warning 语义图标并随文字色悬挂对齐，页面层不加图标。 |
| `src/components/OemConflictHost.tsx` / `OemConflictAction.tsx` | 应用根唯一托管原厂进程退出确认。卡片上的 FnKey 占用状态提供「关闭原厂软件」。交互操作被占用门禁拒绝时直接打开同一确认。工作区变化时撤销未提交的确认。普通确认退出后重新读取，预设确认继续执行已审核的模式目标。 |
| `src/components/ConfigurationAction.tsx` | 在声明模块内呈现组合编辑的预览与应用。初始恢复用修订绑定共享确认。模块拥有自己的校验、执行与回读。暂时锁定提交时保留已显示的预览和恢复入口。 |
| `src/styles/module-layout.css` | 页面内容边界、模块间距与分栏轨道。工作区用 `ccc-control` 纵向排列模块，用 `ccc-module-grid` 组合模块并选择列配置。 |
| `src/styles/configuration.css` | 模块操作区、状态行和配置预览的布局。 |
| `src/styles/control.css` 的 `ccc-panel-head`、`ccc-panel-title` 和 `ccc-control-caption` | 各页共享模块标题行与简短状态文字样式。 |
| `src/components/control/DisplayModeVisual.tsx` | 显示器模式SVG，取实际/明确目标分辨率、方向和频率；当前与目标单独标明，未知显示空状态，不模拟正在扫描的物理画面，档案复用同一绘图 |
| `src/components/control/InputDeviceVisual.tsx` | 触摸板与麦克风 SVG；图形标明设备用途，旁边的简短状态取实际设备快照。 |
| `src/components/profiles/ProfileDetail.tsx`、`ProfileParameters.tsx`、`ProfileCurves.tsx` | 档案应用前的变更、联动和阻止原因，按需展开完整参数。风扇用同类曲线对照，当前曲线用虚线。执行摘要取真实报告。 |
| `src/components/control/KeyboardBacklightVisual.tsx` | 白光键盘的当前保存值与亮度草稿SVG；几何键帽为示意，灯光随明确开关/亮度变化，未知不绘制点亮。编辑页的亮度数字由滑杆展示，示意图补充关闭时保留的档位。档案预览直接显示亮度数字 |
| `src/components/control/PowerSourceVisual.tsx` | 插电/电池/备用/未知来源的语义供电SVG，瓦数只使用后端明确报告，不把PD档位换算为功率；自动规则复用来源图形但明确规则目标 |
| `src/styles/configuration-visuals.css` | 配置领域的绘图尺寸、图形/数值/控件布局和响应，以及扫描纹理的静态样式、循环与减少动效门控；色值仍归tokens.css，表面边框归Surface，不改全局控件外观 |
| `src/components/control/HotkeyEventPreviews.tsx` | 热键已选/已保存事件及恢复事实的唯一组合；草稿范围与操作入口在前，事件事实随后显示，错误及恢复不折叠隐藏，复用各事实组件 |
| `src/styles/tokens.css` | 视觉令牌唯一所有者（颜色/圆角/间距/字阶/动效），`check:boundaries` 强制 |
| `src/components/BrandLogo.tsx` / `src-tauri/icons/icon.svg` / `scripts/generate-icons.mjs` | 品牌标识使用同一 SVG 绘图源。组件隔离实例并接入减少动效，标题栏以 32 px 展示，关于页以 48 px 展示。README 使用生成的桌面 PNG。生成入口同步 favicon 与桌面图标，构建校验资产是否最新 |
| `src/styles/base.css` | reset + 全局硬约束 + 领域样式聚合入口（`@import`） |
| `scripts/ui-contract.mjs` / `check-ui-boundaries.mjs` | 控件边界校验唯一契约：TypeScript/PostCSS AST 检查原生入口、全局私有类及子部件、业务类别名和内联视觉覆盖；`check:boundaries` 随构建执行，负例由 `check:controls` 验证 |
| `src/api/client.ts` | IPC 唯一边界：唯一 `invoke()` 封装 + 领域 re-export 聚合；`check:boundaries` 强制 |
| `src/components/use-toast.ts` | 全局通知唯一实现：命令式 `toast()` 任意模块可调，`useToast()` 供渲染方订阅 |
| `src/components/Toaster.tsx` | 通知唯一渲染 Host，挂载在应用标题栏。摘要显示最高优先级消息和计数，展开后显示完整详情、逐条关闭和清空。摘要外、展开、悬停、聚焦与页面不可见时暂停短时通知。原厂低温电池提醒仍由同一 Host 直接展示和确认，保留后端生命周期。 |
| `src/components/Button.tsx` / `src/styles/button.css` | 主操作用亮蓝实底，次操作用描边表面，轻操作用透明底，图标按钮使用方形热区。按钮原语统一拥有禁用、忙碌、危险态和语义图标 |
| `src/components/Input.tsx` / `src/styles/field.css` | 文本、数字与隐藏文件输入唯一入口，原生属性及 ref 透传；40/32px 高、12px 圆角、边框、禁用、无效和焦点样式全局统一 |
| `src/components/ColorPicker.tsx` / `ColorPickerEditor.tsx` / `ColorSwatch.tsx` / `src/styles/color-picker.css` | 所有自定义 RGB 的唯一入口。可点击色块打开应用内面板，拖选、色相、RGB 和 HEX 同步更新一个受控草稿。面板随色块定位，在视口内避让并随页面隐藏而关闭。只读预设与灯位选择复用色块；颜色格式转换归 `src/lib/color.ts` |
| `src/components/Select.tsx` / `SelectMenu.tsx` / `use-select.ts` / `use-select-position.ts` / `src/styles/select.css` | 普通与搜索选择共用受控契约。菜单从触发器起始侧对齐，按选项内容和可用视口定宽，紧贴字段上下展开；导航行用浅色高亮，当前项右侧勾选。长列表显示滚动箭头，焦点、过滤、禁用与关闭共用同一实现 |
| `src/components/Checkbox.tsx` / `src/components/Radio.tsx` / `src/styles/choice.css` | 复选框及单选输入唯一原语；Checkbox 用于草稿布尔选择，Radio 拥有普通与卡片/分段覆盖输入，保留原生表单语义；即时异步开关仍用 Switch |
| `src/components/SegmentedControl.tsx` / `src/styles/choice.css` | 分段单选唯一结构，受控值与原生键盘行为共享。业务可标记忙碌与待完成选项，暂停激活时保留焦点。需要对已选项显式重选时，由调用者提供回调。 |
| `src/components/Tabs.tsx` / `src/styles/tabs.css` | 内容页签唯一原语，拥有页签列表、面板、选中态和焦点。选中项以蓝色底线和浅色底突出，窄窗可换行。方向键、Home 与 End 移动焦点，Enter 或空格激活。默认仅挂载当前内容，电源区域保活已访问内容。 |
| `src/components/Slider.tsx` / `src/styles/slider.css` | 滑杆统一处理指针拖动与提交，原生 range 提供键盘步进与读屏语义。24px 胶囊轨道、填充、刻度、28px 滑块和当前值标记共用位置换算。性能与散热的常用整数刻度有磁吸，松开范围比吸入范围稍宽，方向键每次移动一个单位。色相选择使用连续色谱。ScaleSlider 提供业务读数、单位与当前值 |
| `src/components/Surface.tsx` / `src/styles/surface.css` | 面板、卡片、信息组、工作区与内嵌区的表面/边框/圆角唯一原语；业务只保留布局，选择卡片的悬停、选中、焦点及禁用状态同源 |
| `src/components/Switch.tsx` | 开关唯一原语：原生 checkbox 提供 switch 语义与键盘行为，CSS 自绘轨道、滑块和焦点；异步保存用 aria-disabled 加输入拦截阻止重复操作，同时保留键盘焦点 |
| `src/components/ScrollArea.tsx` | 滚动容器唯一原语：固定外框、可聚焦原生 viewport 和固定浮层插槽。AppShell 通过 viewportRef 保存和恢复各页面及电源子区的会话滚动位置 |
| `src/components/BackToTopButton.tsx` / `src/styles/back-to-top.css` | 返回顶部唯一实现，AppShell 在工作区固定外框内挂载一次。按钮复用全局次操作样式，按当前视口的实际滚动位置显隐，点击或键盘激活后将焦点交回工作区并滚到顶部 |
| `src/styles/scroll-area.css` | 全局滚动条唯一样式：透明轨道、细滑块，空闲隐藏，滚动、滑块悬停或拖动时显示；尺寸、浅深颜色与空闲延时由 tokens.css 拥有 |
| `src/app/useScrollActivityMarker.ts` | 全局滚动活动唯一所有者：App 根调用一次，被动捕获原生滚动，逐面标记并在空闲 900ms 后清除；卸载清理监听、计时器与标记 |
| `src/components/settings/` | 软件偏好分组、PreferenceCards、系统 FontPicker、ShortcutSetting 与关于；settings.css 只拥有布局和业务内容，控件及容器外观归对应全局原语；settings-previews.css 拥有示意画语法，预览颜色复用 tokens.css |
| `src/app/useAppPreferences.ts` | 已确认偏好快照、读取/保存/重启互斥、错误和原生异步诊断唯一所有者；App 根唯一调用，工作区只组合 |
| `src/api/preferences.ts` / `src-tauri/src/preferences/` | 软件偏好 IPC 与严格文件契约、桌面执行/补偿、系统字体、快捷键及启动窗口配置；日志持久化使用 Tauri 官方日志插件 |
| `src/app/useAutostart.ts` | 开机启动读取、保存、回读、并发锁与错误状态唯一所有者 |
| `ConfirmButton`（components/ConfirmButton.tsx） | 用共享对话框确认单独启动的命令，执行中拒绝重复请求。初始恢复预览在所属模块打开。 |
| `FactList`（components/FactList.tsx） | 键值事实唯一原语，由 `dashboard.css` 定义。行式共享标签列，用弱化标签、同一值列起点和行间距形成阅读顺序。较长标签和值按列宽换行。联动预览和配置差异可用并列网格。 |
| `src/components/control/HardwareParameters.tsx` / `ParameterHelp.tsx` / `parameterDescriptions.ts` | 详情对话框显示真实规格、术语和诊断；字段未知时明确标注，规格、实时值、偏移与限制值分开命名。 |
| `CapacityOverview.tsx` 的内存频率事实 | 内存频率在内存卡统计区与详情弹窗显示，以 MT/s 显示已安装内存条共同报告的 SMBIOS 配置速率；配置速率缺失或不一致时保留未知；系统容量与原厂共享报告分别命名，报告频率和参考频率直读 MHz，参考值不作为硬上限或比例分母，说明共享报告延迟与 DDR 有效速率的区别 |
| `api/hardware.ts` | 硬件快照类型与订阅唯一封装（`hardware-snapshot` 事件 1Hz） |
| `src-tauri/src/hal/overview.rs` / `src/app/useSystemOverview.ts` | 系统只读概览契约：CPUID 身份、物理内存、SMBIOS 内存速率、固定磁盘卷；可见页的轮询与再进入共用串行读取，旧请求结束前不发新请求，过时代次不回写；同一次采样更新当前读数和最近五分钟内存历史，读取失败显示未知并等待下次轮询 |
| `src/components/control/HardwareArtwork.tsx` / `ProcessorArtwork.tsx` / `GraphicsCardArtwork.tsx` / `ModuleArtwork.tsx` / `CapacityOverview.tsx` | 品牌标识、按实物参考重绘的四类部件与容量卡：CPU 只传单一品牌，由 ProcessorArtwork 派生顶盖与文字；非 CPU 图形不接收品牌参数。GPU 使用桌面双风扇整卡，内存使用鳍片马甲，存储使用 M.2 标签/缺口/安装孔；装饰使用 aria-hidden，不把参考商品型号当作本机事实，容量有真实分母才绘制使用量 |
| `src/components/control/` | HardwareReadings/MetricReadout/SensorTemperature 图标读数与温度面积图、可拖动曲线、数值滑杆与短变更预览。按分类工作区组合。拖动过程更新预览。 |
| `src/components/control/FanCurvePlot.tsx` / `src/styles/control.css` | 两点和十级 RPM 编辑图共用刻度、网格、单位与图例。绘图区同时作为曲线、手柄和指针换算的坐标基准。 |
| `src/components/control/RpmCurveEditor.tsx` / `rpmCurve.ts` | 十级 RPM 阈值编辑与草稿约束；复用 CurvePointInputs 的数值控件，纵轴是该风扇最大 RPM 百分比，按阈值画阶梯 |
| `src/styles/monitoring.css` | 温度和内存历史、CPU/GPU 图标读数、卷容量条与比例环的唯一样式；档案功耗、电压和频率使用带单位的数值事实，不以人为上限绘制比例 |
| `src/components/performance/GpuTuning.tsx` | GPU 偏移区唯一实现，NVAPI 类型化读写、回读和恢复，由性能与散热页组合 |
| `src/components/control/KeyboardLighting.tsx` / `KeyboardEffectEditor.tsx` / `useKeyboardLighting.ts` | 灯效、亮度档按钮及灯区开关直接提交。色环和白光滑杆保留本地草稿，卡内应用和恢复。 |
| `src/components/control/KeyboardLightingPreview.tsx` / `LightingHueWheel.tsx` / `src/styles/lighting.css` | RGB 全键盘背光示意，色环与中心 `ColorPicker` 编辑同一草稿。键位几何复用 `perkey-layout.ts` |
| `src/components/control/LightingIdleTimer.tsx` / `lighting-option-values.ts` | 闲置关灯时分秒草稿与有效秒数换算，提交和恢复复用灯光选项契约 |
| `src/components/control/TopCoverEditor.tsx` | 独立顶盖当前/草稿的亮度和颜色；亮度0关闭，复用灯光策略确认及恢复，区别于多区顶盖 |
| `src/components/control/LightingOptions.tsx` / `useLightingOptions.ts` | 开机覆盖和各灯区开关直接提交，只使用已回读的其余字段。时长、颜色与顶盖亮度在卡内编辑、应用和恢复。 |
| `src/components/control/LightBarSettings.tsx` / `LightBarEditor.tsx` / `lightbar-values.ts` / `useLightBarSettings.ts` | 独立灯条的开关、可用模式和 X58 七色直接生效。缺少已保存颜色的模式在选色后切换。连续参数在卡内应用，型号变动显式重载草稿。 |
| `src/components/control/MultiZoneLighting.tsx` / `MultiZoneEditor.tsx` / `useMultiZoneLighting.ts` | 四区键盘的开关和模式直接提交。配色和滑杆保留草稿并在卡内应用，读取不初始化保存值。 |
| `src/components/control/MultiZoneColors.tsx` / `src/components/profiles/ProfileMultiZonePreview.tsx` | 「键盘与灯光」和档案共用多区颜色示意，按实际能力显示灯区；模式/速度/亮度按单位展示，不冒充物理回读 |
| `src/components/control/PerkeyLighting.tsx` / `PerkeyPatternEditor.tsx` / `usePerkeyLighting.ts` | 逐键 RGB 开关、灯效、方向及动态配色类型直接提交。逐键颜色、亮度与速度在卡内应用和恢复，按键响应另报运行状态。 |
| `src/components/control/PerkeyKeycaps.tsx` / `src/lib/perkey-layout.ts` / `perkey-layout-data.json` | 原厂布局事实生成的键帽与LED对应关系；大键同时选择全部对应灯位，未知布局明确使用灯位矩阵。外观由全局Button的keycap变体拥有，不复制原厂UI样式 |
| `src/components/control/KeyboardSettings.tsx` | Fn 锁、Win 锁与 Fn/Win 交换根据固件能力逐项提交。请求使用最新回读的其他字段 |
| `src/components/control/TouchpadSettings.tsx` | 系统状态和保存偏好分开展示。启用、关闭直接提交并作用于当前设备集合，旧修订的请求被拒绝 |
| `src/components/profiles/ProfileTouchpadPreview.tsx` | 档案触摸板设备集合、系统目标及保存偏好，左右模块预览；不以保存值冒充系统状态 |
| `src/components/control/MicrophoneSettings.tsx` | 状态摘要与详情中的逐端点状态来自同一回读。全部静音和取消静音直接提交 |
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
| `src/components/control/HotkeySettings.tsx` / `useHotkeySettings.ts` | 原厂热键范围与屏显草稿、规则预览（分区、颜色循环、亮度循环/反馈、只读状态通知及摄像头报告同步分别独立授权）、接管确认、临时停止及初始软件策略恢复；亮度事件说明RGB主动循环与白光报告同步的区别，产品文案不暴露原始型号编号。后台操作复用领域事务，硬件初始恢复保留在对应模块。状态通知标为原厂报告，最后结果与是否显示桌面提示分开；键盘灯快捷入口打开「键盘与灯光」，聚焦键盘灯光区域后确认消费 |
| `src/components/OsdHost.tsx` / `src/styles/osd.css` / `src-tauri/src/osd/` | 桌面屏显唯一Host：独立透明、置顶、无焦点、穿透窗口；展示已提交操作结果、已授权原厂状态报告或明确预览样例。序列号隔离旧消息与计时任务，窗口只允许读取屏显状态；不挂载主应用或硬件轮询 |
| `src/components/control/PlatformPower.tsx` | 选择原厂支持的 PD 档位直接提交并回读，初始等级卡内恢复。Deep S5 只读，适配器瓦数和 PD 档位分别展示。 |
| `src/components/control/UsbPowerPolicy.tsx` | 改动只提交所选的插电或电池字段。原厂建议一次提交两种供电策略，初始恢复预览显示三个标准计划的初始值并保留活动计划，恢复受阻的原因留在卡内。 |
| `src/components/control/LanPolicy.tsx` / `useLanWakePreview.ts` / `lan-values.ts` | 网卡节能直接预检并请求系统授权。组合唤醒补丁和逐字段初始恢复在对应卡片预览后执行。 |
| `src/components/control/FanDust.tsx` / `DustScheduleEditor.tsx` / `useFanDust.ts` | 除尘星期、整点和唤醒触发直接提交并回读。一次除尘与恢复初始计划分别确认，运行状态由固件报告。 |
| `src/components/control/DisplaySettings.tsx` | 选择活动显示器的刷新率或翻转主屏，直接开始 15 秒试用。在屏幕卡内保留或取消。自动试用和档案试用指向各自的全局入口 |
| `src/components/control/AutomaticRefreshSettings.tsx` | 即时提交自动刷新策略，停用恢复初始屏幕与原厂保存字段；开关表示是否仍持有启用记录，已暂停或事务未完成时保持打开，关闭即恢复。当前、低刷与恢复频率用离散读数展示，监听失败和外部修改在卡内反馈。读数面板内的扫描纹理 SVG 拥有动态背景装饰：画幅、扫描线、扫掠光束与行进光点，对辅助技术隐藏并忽略指针 |
| `src/components/control/useDeviceSettings.ts` | 独立模块共用读写生命周期。一次提交锁定所属模块，旧读取失效，写入后回读，操作错误和读取错误分别保留。 |
| `src/components/control/SupplyModeSettings.tsx` / `SupplyModeRecovery.tsx` / `useSupplyMode.ts` / `src/styles/supply-mode.css` | 原厂自动模式的完整初始恢复；独立启用/继续/暂停/恢复、失效读请求、模块边框和左右布局，通知结果由后端拥有 |
| `src/components/control/gpu-values.ts` / `power-plan-values.ts` | GPU原始锁/偏移及目标、Windows计划/链路节能名称的共用展示；未知保留未知，不建立另一份执行规则 |
| `src/components/AutomaticDisplayTrialHost.tsx` | 自动刷新率试用确认的唯一全局Host，应用根挂载；任何页面或隐藏到托盘后唤起时均可看到当前至目标、剩余时间和保留／恢复。保留前由后端复核供电和模式，超时由独立监护恢复；手动与档案试用不在此Host重复确认 |
| `src/components/control/GpuPowerSettings.tsx` | 独显电源选择直接请求 Windows 授权并回读，保留活动输出保护。关断暂停 GPU 轮询和调校 |
| `src/components/control/GraphicsModeSettings.tsx` | 按固件支持的按钮直接发送模式选择。固件现值、待重启目标及重启后确认分开展示，待重启时由用户显式发起重启 |
| `src/components/control/BatterySettings.tsx` | 充电开关和有效阈值直接提交，缺少合法组合时等待选齐。节能、校准和充电写入互斥，初始恢复在卡内。 |
| `src/components/control/BatteryOverview.tsx` / `BatteryArtwork.tsx` / `src/styles/battery-visuals.css` | 同一摘要行显示电池图、系统电量和供电及充电状态，有系统估时才显示预计剩余时间。填充只取实际百分比，未知显示空电池，0%显示数值。正在充电，以及外接电源、100% 且明确未在充电时，填充用语义绿。充电标记取明确的充电报告。 |
| `src/components/control/EnergyPolicySettings.tsx` / `EnergyPolicyEditor.tsx` / `useEnergyPolicy.ts` | 开关直接切换已保存计划，停用恢复充电策略。对应编辑器保存日期、时间段和阈值组合，卡内恢复初始计划。 |
| `src/components/control/BatteryCalibrationSettings.tsx` / `useBatteryCalibration.ts` | 电池校准的当前阶段、真实状态与耗时、接管及恢复目标预览、独立开始和停止及恢复确认。任务生命周期由原生校准领域拥有 |
| `src/components/control/StartupProfilePicker.tsx`、`useStartupProfile.ts` | 取消启动档案直接保存。选择新的启动档案时，在授权确认中经「查看档案参数」文字按钮预览完整档案，授权绑定文件和模式联动。最近启动结果从执行状态读取。 |
| `src/components/control/OemModeEffects.tsx`、`energy-values.ts` | 档案、供电条件与热键循环共用的原厂模式联动预览。插电偏好、固件节能、风扇、偏移、Turbo/DTT、ASPM与系统计划按语义展示，节能值与硬件管理共用唯一格式化函数 |
| `src/components/control/OemModeGpuEffects.tsx` / `GpuModeFacts.tsx` | 模式GPU当前/目标、独显动作时序、驱动范围和首次恢复来源；宽屏左右、窄屏纵排，Surface保留模块边框和间距。GpuModeFacts唯一展示原始锁频、偏移及未读取原因，供电与明确选择共用；离散模式不画比例，关闭独显时不伪造频率 |
| `src/components/control/ModeCycleFacts.tsx` | 热键循环的完整模式候选、插电/电池频率及独显阶段预览；左侧实读、右侧按模式分组，原始频率与驱动范围复用GpuModeFacts |
| `src/components/control/GpuPowerVisual.tsx` / `GpuNodeStages.tsx` | GPU精细插画与短电源状态、已预览节点阶段图；复用HardwareArtwork显卡SVG，离散状态不绘制比例，阶段仅由真实预览请求产生，诊断与完整参数可展开 |
| `src/components/profiles/ProfileModeAuthorization.tsx` | 后台档案完整模式许可预览：复用GPU供电目标表和语义图标，展示未来AC/DC频率、允许的自动关断条件与驱动延后状态；长规则可展开，不把当前raw值作为长期许可 |
| `src/components/control/HotkeyCurrent.tsx` / `hotkey-actions.ts` | 热键运行状态与暂停、规则名称和说明的唯一事实源；只展示已选范围、当前接管或仍有恢复记录的详细能力，恢复入口不因取消草稿选择而消失 |
| `src/components/control/OemPerformanceOptions.tsx` | 娱乐与性能分别显示寂静模式，并按已核对的固件命令提供 Turbo 风扇和动态调校 DTT。娱乐档应用性能选项需确认当前档的目标值及风扇联动。娱乐档从自定义固件曲线启用 Turbo 时预览切换到自动风扇，确认后可恢复初始曲线。Turbo 与寂静档互斥，激活 DTT 核对已启动的设备驱动。 |
| `src/components/control/PresetArtwork.tsx` / `src/styles/mode-presets.css` | 概览与性能与散热共用的模式插画、渐变档名、嵌入布局及回读切换动画。SVG 渐变按实例隔离，主图隐藏于辅助技术并忽略指针。 |
| `src/components/control/ModeArtwork.tsx` | 供电与热键候选的静态模式 SVG。集中维护模式的视觉分类，静音增强与安静共享分类。 |
| `src/app/useControlModel.ts` | 各调节域刷新代次唯一所有者。性能与散热的单项操作回读受影响的控制域。热键及档案应用 id 或阶段修订触发事实回读。已编辑草稿保留，未编辑草稿采用新事实。独显操作后刷新 GPU 调校。 |
| `src/app/useProfileReads.ts`、`useProfiles.ts`、`useProfileReferences.ts` | 档案列表、手动预览和后台授权各自按请求与页面有效性更新。托盘请求在写入后消费。规则引用读取启动和电源的已有事实，修订不一致时指向电源页重新确认。 |
| `src/components/profiles/ProfileGpuPowerPreview.tsx` | 档案独显节点与自动关断策略；左右分组、独立开关事实和目标，身份归折叠诊断，明确软件风扇退出及开启后驱动/温度复核，不用比例图表示布尔状态 |
| `src/components/profiles/ProfilePreview.tsx` / `ProfileDevicePreview.tsx` | 档案风扇与参数预览，含已读取的原厂联动目标；设备卡复用LightBarPreview/型号摘要与除尘星期语义，以左右分组卡展示。离线允许查看导出，缺少联动预览时禁止手动应用 |
| `src/components/profiles/ProfileGraphicsModePreview.tsx` | 档案MUX目标、当前启动报告及已知待重启选择左右分组；明确请求接受与实际切换的边界，不提供自动重启或伪比例 |
| `src/components/profiles/ProfileKeyboardPreview.tsx` / `control/keyboard-values.ts` / `control/lighting-option-values.ts` | 档案键盘功能、RGB/白光及外围策略卡；复用编辑页字段与摘要，左右布局、独立开关/亮度和灯区色块示意，不复用禁用编辑器或伪造比例 |
| `src/components/profiles/ProfilePerkeyPreview.tsx` | 档案逐键灯效只读键帽图；复用原厂布局及LED索引，未知布局显示矩阵并说明；动态效果展示配置色彩和方向，不伪造实际运行画面，按键响应仅说明应用后的启动意图 |
| `src/components/profiles/ProfileKeymapPreview.tsx` / `src/lib/keymap.ts` | 档案映射/宏的设备范围、启用与程序监听意图；复用按键名称和完整动作摘要，左右卡片、设备图形和可展开的全部目标，不执行录制或宏、不把保存意图当作过滤器回读 |
| `src/components/profiles/ProfilePlatformPreview.tsx` / `control/platform-values.ts` | 档案PD与USB目标；共用等级和标准计划名称，左右分组、离散等级选择标记与逐计划AC/DC事实，不把档位或开关伪造成连续比例 |
| `src/components/profiles/ProfileBatteryPreview.tsx` / `control/EnergyTimeline.tsx` | 档案充电及节能目标；左右模块、明确阈值环与原始关闭状态、星期和真实24小时时段。时间条由电源页的节能编辑器与档案共用，预览只展示配置，不触发校准或假定物理充电效果 |
| `src/components/profiles/ProfilePerformancePreview.tsx` / `control/OemPerformanceOptions.tsx` | 档案 Turbo/DTT 保存偏好与实际模式语义。共享字段名称、状态和风扇联动说明。手动应用预览由 preview_profile_application 提供，后台规则拥有独立授权预览。 |
| `src/components/profiles/ProfileLanPreview.tsx` / `control/lan-values.ts` | 档案逐网卡驱动目标、节能及唤醒语义；左右模块卡片，缺失目标明确表示恢复未设置，原始类型和身份归折叠诊断；复用LAN字段名称及状态文字，不把驱动字段显示为比例 |
| `src/components/profiles/ProfileDisplayPreview.tsx` | 档案每台显示器的身份/几何与当前至目标刷新率；左右模块卡片，离线标明当前未知，不用比例表达频率；启动预览说明仍需试用保留 |
| `src/app/profile-application.ts` / `src/components/ProfileApplicationHost.tsx` | 档案应用结果与全局显示确认的唯一前端状态；App唯一挂载，整组15秒试用明确保留后继续其余设置，取消/超时/失败分别展示，不把待确认显示为全部成功 |
| `ConfirmButton` 禁用态 | disabled/busy 锁定确认入口，自身提交期间显示进度。确认文本跟随当前草稿，执行中拦截重复写入 |
| `src/app/navigation.ts` | 页面注册唯一清单；页面标识唯一来源 |
| `src/app/AppShell.tsx` | 外框唯一所有者；页面不拼装标题/导航/骨架 |
| `src/app/SideNav.tsx` / `src/app/useSideNavIndicator.ts` | 侧栏唯一实现，拥有顶部折叠入口、导航按钮与收起提示、指示条和底部设置。页面清单来自 navigation.ts |
| `src/components/Icon.tsx` / `src/styles/icons.css` | Lucide 与 Phosphor 唯一接入点，拥有语义图标、尺寸、颜色继承与导航字形动效。装饰图标对辅助技术隐藏，独立状态可传 label |
| `src/app/WindowControls.tsx` | 集成标题栏窗口按钮唯一实现，共享置顶偏好，监听真实最大化状态，失败使用全局通知 |
| `src/api/window.ts` | 桌面窗口类型化 API 唯一封装；经 client.ts 聚合，窗口操作由 commands/window.rs 拥有 |
| `src/app/AppWorkspace.tsx` | 工作区挂载唯一所有者（keep-alive 接线） |
| `src/lib/appearance.ts` | 已确认软件外观应用与系统主题监听唯一入口；真实桌面缩放由原生 WebView 设置，浏览器验收只镜像该比例 |
| `src/lib/startup-page.ts` | 最近一级页面的本地布局状态与启动解析；与侧栏布局状态独立，均不保存机器配置 |
| `src-tauri/src/command_registry.rs` | 全部 IPC 命令唯一注册清单 |

新增全局原语必须先在本表登记，再写实现；模块只能在本表原语上组合业务，不复制基础控件结构、不创造平行组件。

## 图形界面

- 父级网格统一分配模块的宽度和间距。单列模块沿同一内容区的左右边缘拉伸。同行卡片按轨道分列，由最高内容决定共同的顶部和底部边界。窄窗口按顺序换行并使用自然高度。设备模块的主要操作行靠底对齐。
- 模块根节点用静态类声明内部布局。宽度、行内外边距、自身对齐和轨道定位由父级布局负责。`check:boundaries` 检查受管模块及其样式类，工作区浏览器测试检查实际卡片边缘。
- CPU、GPU、内存和存储卡片保留实况数字与有真实分母的图形。规格、完整参数和术语由文字按钮打开共享详情对话框。CPU 标称频率、GPU 当前功耗上限和磁盘卷按各自意义命名。
- 控件、预览、档案、启动摘要和诊断统一使用持续功耗上限、短时功耗上限、核心/核显电压偏移、核心/显存频率偏移。前端风扇模式名称由 `api/fans.ts` 提供，性能选项共用 `api/power.ts` 的 Turbo 风扇和动态调校 DTT 名称。原厂风扇屏显与控制器使用「寂静模式」。CPU/GPU 部件简称、标准单位与协议标识沿用各自契约。

- 概览只拥有实时监控；设备编辑器分布到注册的领域工作区，每个编辑器只有一个挂载点。领域工作区沿用控制模块样式，active由对应页面透传；app层唯一拥有历史订阅、跨域修订和逐页滚动，草稿随工作区保活。
- 三区 RGB 键盘的灯效、色环及亮度排在全键盘预览上方，宽窗口横排，窄窗口逐行布局。同色点亮全键盘，分区和呼吸按三区颜色与开关示意，固件配色只显示键位。色环支持点击、拖动和键盘调整，点击中心色块打开全局选色面板。亮度可直接选择关闭或 1–4 档，选择后立即更新键盘示意。草稿经页面预览和确认后提交，未知类型保留诊断和重新读取。
- 自定义灯光色块使用同一应用内面板。色域、色相和数字输入同步，RGB 接受 0–255 的整数，HEX 接受六位十六进制，未完成或无效的输入失焦后回到有效值。关闭面板保留有效草稿。设备限定的固定颜色继续按该设备的枚举值选择。
- 独立顶盖由能力声明后显示，复用外围灯选项事务；左侧当前亮度与颜色示意，右侧亮度和颜色草稿。亮度0关闭并保留色值，卡片有独立边框与间距，窄屏纵向排列；与多区顶盖区明确区分，档案及启动预览保持同一含义。
- 灯光策略与外围灯区独立分组。闲置关灯、开机覆盖、Logo 和灯条的名称和开关在各自卡片的首行对齐。闲置等待时长在开关行下方以时、分、秒编辑，Logo、灯条和顶盖按能力显示。窄屏纵向排列。预览按总秒数比较时间，仅显示本次支持且有变化的项目。恢复目标明确列出，键盘模式恢复不得覆盖这些选项。
- CPU、GPU 和系统内存卡头的插画与品牌标识居左，旁边纵排标题、副标题、型号和文字详情入口，按部件显示适用内容。
- 概览与风扇设置沿用 Apple 人机界面指南的对齐、分组与图标一致性。宽窗趋势图与读数使用同一内容边距，辅助图标用无底色的单色线条，曲线表面和留白区分图表与其余内容。[布局](https://developer.apple.com/design/human-interface-guidelines/layout) · [图标](https://developer.apple.com/design/human-interface-guidelines/icons)
- CPU/GPU 卡片依次排列部件身份、温度趋势与指标网格。频率、利用率、风扇占空比和显存使用统一的语义图标、标签和带单位的实读数值，显存指标显示已用量，总量可从 GPU 规格或读屏读数获取，风扇转速归性能与散热页。频率以 MHz 直读，CPU 使用与 Windows 处理器效用同一次采样的性能百分比乘以同组 `Processor Frequency` 标称基频，GPU 使用显卡核心频率。`SensorTemperature` 显示当前温度、窗口峰值以及最近五分钟的带填充折线，使用相对时间刻度、固定 20–110°C 标尺及 95°C 高温参考区。以最新采样为窗口右端，采样缺值或间隔超过 5 秒时断开折线和填充；≥95°C 的读数与标记标红，悬停显示该采样点细节。概览不常驻采样时刻，连接与异常状态按需显示在模式卡下方，硬件采样失败时提供重试。
- 系统内存以通栏的 0–100% 面积图显示最近五分钟的物理内存使用率，和 CPU/GPU 趋势共用图头、网格及相对时间刻度。图头显示当前使用率和窗口峰值，图下等宽排列已用、可用、总容量和频率，窄窗两列。可见时沿用完成后 5 秒的概览轮询，缺少容量或停采时留出缺口；容量未知时独立显示可用的频率读数。每个磁盘卷以实际卷标和盘符标识，共用容量条，下方显示可用和总容量。容量未知时显示状态。前端容量格式以字节为输入，按 1024 进位显示 B、KB、MB、GB、TB 等单位，概览与详情共享换算。
- 风扇编辑图使用低对比度细网格，纵轴从 0% 到 100%，每 20 个百分点一格。横轴以 20°C 为主刻度并标出绘图范围的两端。固件两点图默认 30–100°C，低温锚点低于 30°C 时向下扩到整数十度。十级 RPM 图用 0–100°C，纵轴标明占空比或目标转速，十级目标 RPM 以阶梯呈现。实线与轻填充表示主曲线，填充从主曲线起点闭合到零线，草稿变化时用虚线对照当前设置。两点曲线的点位标签在悬停、聚焦或拖动时显示，选中的点与输入对应高亮。宽窗点位输入与图并排，窄窗排在图下。曲线估算标于图例，实况区显示所选风扇的有效实测温度和转速。拖动与数字输入共用草稿。
- 档案详情展示已保存的软件曲线。应用软件曲线须通过固件控制权回读门控。风扇设置展示接管状态与未确认归还的原因。
- 十级表的温度阈值严格递增，新的手动目标转速为 20–100% 且单调不降，末级 100%。选择自定义启用已有原表并保留停转级。档案按每把风扇分图。
- 控制区用表面色与留白分组，移除逐卡描边。滑杆仅保留操作轨道，边界值在交互时显示；主应用与次级恢复有明确层级。
- 图表以实际采样时间定位，缺失值保留断点，过期数据明确标记。没有硬件数据时不生成示例读数。
- 日常文案以数值、单位、短标签、状态和动作为主。实现细节与原始读数归诊断弹窗。
- 使用草稿确认的编辑器跨页面保留草稿，应用前展示变更差异。写入期间禁止重复操作，恢复调用后端记录的原始状态。
- CPU 卡在功耗上限与电压偏移两组前切换「立即应用／保存设置」，分别呈现 MSR 回读与持久化值；两者的草稿独立保留，保存结果不表述为实时生效。PL1/PL2 联动保持有序。GPU 卡将核心与显存频率偏移排在同一组。宽窗并列参数组或同组控件，窄窗依次排列。两卡标题下方提供文字「说明与诊断」入口。
- GPU 单项调校缺少管理员权限时仍展示读取到的偏移，同时禁用编辑、应用和恢复。当前 Windows 账户具有同账户提升资格时提供「以管理员身份重启」，经确认请求系统授权。其他账户在卡内显示原因。授权取消或交接前失败在卡内反馈，交接后的启动失败显示原生诊断。回滚失败后重新读取实际偏移，保留草稿和事务错误。读不到当前值时显示原因及重试入口。
- 一次 CPU 确认提交全部变更字段，失败恢复整组；档案预览按「立即应用 · CPU」「保存设置 · CPU」「立即应用 · GPU」分组，用语义量表显示数值，电压编码换算为 mV。手动确认绑定完整预览，同名内容变化也重置确认；拒绝旧预览时清除旧执行结果，恢复失败有明确提示。
- 散热页点选固件模式直接提交。曲线和加速偏移在松开指针或方向键时提交，点位数值在回车或失焦时提交。曲线提交同时启用自定义，互斥时显示锁定原因。面板使用 EC 即时档位。确认自动或寂静档、能力和实际偏移后开放滑杆，未上报时保留滑杆位置和禁用原因。提交绑定档位和电源档记忆槽，后端持写锁核对状态。恢复初始与固件默认独立确认，提示固件默认会清零支持的偏移。各通道的温度和转速使用本通道读数。
- 性能与散热回读期间保留面板与提交草稿。忙碌时临时锁定操作，保留控件位置与读数。滑杆、开关和曲线点在暂时锁定时保留焦点。读失败的设置保留上次内容供查看并锁定写入。过期的风扇温度和转速标记为上次采样值。
- CPU 功耗须有有效且有序的成对起点才可编辑。保存设置优先使用已保存值，对未上报字段使用同项固件出厂值。缺少有效起点时锁定功耗编辑，电压偏移仍可独立提交。
- OEM 电源模式预览列出原厂风扇、受支持的偏移、PCIe 节能与 Windows 电源计划目标。确认绑定整份联动目标，原厂保存策略变化后重新读取确认。静音增强仅在能力页声明时出现。

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
5. EC / MSR 写入必须经 HAL 契约，可恢复的编辑按所属模块契约提交。草稿型编辑经预览和确认执行，散热页的固件控件以用户完成调整为提交点。安全语义见 `docs/design.md` §4。
6. 不引入路由库 / 状态管理库 / UI 框架 / CSS 框架，除非决策笔记先行并修订本文。
