# Agent Note: 电源模式的原厂联动与完整恢复

Status: implemented

## 问题

路线表的性能调校仍有代码缺口：只发送 cmd121/sub25，没有 FnKey.OnChangePowerMode 的分模式风扇、ASPM 和 Windows 电源计划联动，Quiet Plus 也被硬编码拒绝。用户明确要求继续完成全部代码，不以真机验收缺失停工。

## 决定

2026-11-18 模式选项扩展。原厂 Turbo 风扇和 DTT 通过性能模式命令激活。娱乐模式允许独立编辑并保存这两项性能偏好，保持当前娱乐模式及其风扇。性能模式在固件声明 SlowFan 时提供寂静模式，调用已有固件风扇档 8。2026-11-19 核对原厂后将温度监护限定在软件曲线，预设和性能寂静模式直接交给固件执行。支持分模式记忆时复用性能槽，原厂娱乐模式的 Noiseless 偏好独立。寂静与 Turbo 在同份草稿互斥，模式应用前显示最终有效的风扇目标。新增设置复用模式选项修订校验和首次恢复快照。

2026-11-18 预设视觉改用简洁的内联 SVG。电池与叶片、月牙、手柄、仪表采用透明画板与统一圆角描边，静音增强复用月牙。移除渐变、圆形底托、星点及图形缩放。模式卡片、供电预览和热键循环共用 `ModeArtwork`。系统概览与性能调校采用整行大卡片和普通勾选标记。

2026-11-19 按 Apple 的中心构图与简单背景、Material Symbols 的小尺寸辨识度、Fluent Emoji 的熟悉轮廓重新生成四张预设插画。iOS 与 macOS 专注模式以图形配色区分状态。保留玉绿叶片电池、钴蓝月牙、紫色手柄和铜橙仪表，统一哑光珐琅、左上柔光与低对比单色背景。使用内置 ImageGen 独立生成四幅原图，编码为 768×384 的 WebP。静音增强共用月牙，`ModeArtwork` 供概览、性能调校、供电预览和热键循环共用。

[Apple 图标指南](https://developer.apple.com/design/human-interface-guidelines/app-icons) · [iOS 专注模式](https://support.apple.com/en-euro/guide/iphone/iphd6288a67f/26/ios/26) · [macOS 专注模式](https://support.apple.com/en-euro/guide/mac-help/mchl613dc43f/mac) · [Material Symbols](https://developers.google.com/fonts/docs/material_symbols) · [Fluent Emoji](https://github.com/microsoft/fluentui-emoji)

每张图的提示词由对应的主体段和同一构图段组成。

```text
[eco] Use case: stylized-concept. Subject and palette: A jade-green horizontal battery with big friendly radii and a small integral right terminal. One simple pale mint leaf emblem is inset flush in the front face, with a single gently curved vein. A light muted mint studio field with subtle shading. Deep jade silhouette, cooler mint face details.
[quiet] Use case: stylized-concept. Subject and palette: A broad, solitary cobalt-blue crescent moon with solid mass, a generous open crescent facing right, gently rounded pointed tips, one pale blue bevel highlight and the deeper blue enamel face. A subdued pale blue/periwinkle studio field. The crescent is the only shape.
[entertainment] Use case: stylized-concept. Subject and palette: A compact, original purple game controller with a bold symmetric two-grip shape, one broad pale directional cross on the left and four small unlabeled lilac button domes on the right. No sticks, cables or brand-specific pattern. A muted lilac studio field. Deep plum at the edge and clear violet in front.
[performance] Use case: stylized-concept. Subject and palette: A sculptural copper-orange speed gauge with a thick open semicircular dial, a few equally weighted ivory-orange inset marks, one bold short needle pointing up-right, and an integral round pivot. Use a simple front silhouette without a surrounding instrument housing or numerals. A muted pale apricot studio field with soft depth, rich burnt-orange enamel.

Original product illustration for a premium Windows laptop power-mode picker. A wide landscape banner, target 2:1, with an uninterrupted matte studio backdrop. Show one centered, chunky low-relief object made of satin enamel, with fine broad bevels and a shallow darker edge, viewed almost head-on with a slight elevation. Soft, broad light from upper left, quiet short shadow. Clean, bold silhouette and very few broad details, legible in a tiny 40-pixel thumbnail. Give the single object an optical height around 54% of the canvas and keep it inside the center safe area for cropping. Design the composition as a complete edge-to-edge illustration. No container, no text, no letters, no numerals, no logo, no border, no badges, no extra scenery, no particles, no glow, no texture noise.
```

2026-11-19 预设插画改为内联 SVG，沿用前版四种主体及珐琅材质。共用组件负责静态绘图和实例内的渐变引用。性能调校按卡片悬停播放叶片轻摆、月牙浮动、手柄按键下沉或仪表扫针，移开即恢复静态。系统或应用设置启用减少动效，以及触控和只读预览场景，均显示静态图。移除 WebP 资源引用。模式切换仍由卡片单选控件负责。

2026-11-19 预设选择改用单层中性色卡片与同一套 48 × 48 SVG。参考 G-Helper 的档位层级、Tabler 的轮廓和 IBM 的光学对齐方法，重画电池与叶片、月牙、手柄和仪表。宽窗图形与名称同排，窄窗两列并在卡内上下排列。图形共用描边，淡色只用于局部填充。先检查浅深色与窗口宽度，再让叶片展开、月牙内弧提亮、手柄键短按和仪表指针绕轴。移开复位，概览与联动缩略图复用静态绘图。

2026-11-20 性能调校用居中的等轴芯片 SVG 和分段单选器取代预设卡片及其悬停动画。直接选择时在目标按钮反馈切换进度，主图与已选项跟随设备回读。已知模式变更播放一次渐变、芯片抬升和扫描高光，首次读取与未知状态显示静态画面。动效遵循系统与应用设置。静音增强共享冷蓝材质，保留独立档名。沿用既有固件可用性、同档重选、冲突确认、失败回读与恢复事务。

2026-11-20 概览顶部用宽幅只读卡片共用同一芯片、配色和回读切换动效。芯片直接嵌入卡片中央，档名居中放在芯片下方。短窗压缩图形。采样为空、未知或停滞时显示中性静态芯片。供电与热键候选使用原尺寸的静态模式图标。

- 2026-11-18 预设卡片作为直接切换入口，激活时提交后端返回的完整模式目标及修订。移除模式草稿、差异区及应用按钮。事务期间显示切换状态并暂停再次激活，以设备回读决定当前模式。失败回读并显示原因，再次激活才继续执行。明确重选当前模式仍可应用插电偏好；首次加载和重新读取均只读。后端沿用资格检查、授权、执行锁、失败补偿及首次恢复，恢复设置继续审核独立目标。

- 2026-11-19 预设切换遇到已核验身份的原厂 FnKey 时，弹窗确认「关闭并切换」。确认绑定当次模式目标、进程 ID 与创建时间。后台重新核验 Windows 包、应用 ID、程序映像和会话，请求原厂进程正常退出，等待进程结束后再走完整模式事务。其他原厂控制进程、变化的身份或模式目标、退出失败均阻止执行。此操作仅用于手动切换预设，保留底层服务、驱动与原厂启动配置。取消与离页放弃待确认请求。

- 2026-11-20 预设的「关闭并切换」由应用根的共享弹窗呈现。其他模块可独立确认退出并重新读取。见[原厂控制进程冲突确认](2026-11-20-oem-conflict-dialog.md)。

- 2026-11-20 应用户要求完整移除模式域手动初始恢复，无兼容层、无过渡入口：前端删除 `OemPowerMode` 的恢复按钮、详情中「模式恢复暂不可用」分区、`OemModeRestore`/`OemModeGpuRestore` 组件、`useOemPowerMode` 恢复路径、`restoreOemPowerMode` 封装、`OemModeRestoreTarget` 类型，以及读取载荷中的 `restore`/`canRestore`/`restoreBlockedReason`。后端删除 `restore_oem_power_mode` 命令与注册项、`powerctl/mode/restoration` 模块、`workflow::RestoreTarget`（含 `bind_system`/`bind_gpu` 与 GPU 恢复预览）、`system::RestorePlan`、`RawState::restore_identity` 及两个仅服务该预览的 GPU 恢复复核方法。`ModeProfile.energy` 保留，因为供电模式六条件证据仍依赖它；失败补偿仍由 `Snapshot`、`recover_snapshot` 与 `restore_checkpoint` 承担，供电、档案、启动与热键仍复用同一事务与补偿链。相关测试改为断言首次基线内容或走补偿入口，纯手动恢复用例删除。

- 2026-11-19 真机确认关闭 FnKey 后，模式写入与温度采样同时停滞。预设与模式循环的许可回调曾在 DCHU 锁内重复查询除尘，试图再次获得同一锁。公共模式事务由 HAL 写锁串行化，所有调用方许可在 DCHU 锁外检查。DCHU 锁只保护性能偏好、插电偏好、模式命令与保存、EnergyStar 等设备操作各自的读写，保留写入间的许可、EnergyStar 写前核对与写后回读及失败补偿。除尘检查由模式事务与风扇域负责。

- 删除已无调用者的StoredModeSnapshot单计划system_summary辅助入口；当前消费者统一使用逐计划system_plan_values，不保留旧摘要访问路径。

- 以本机 FnKey Features/OnChangePowerMode/FAN.Load_FanMode_forFn_3 为事实源；page7[17] bit7 开放模式4（即时命令0），保留 Turbo/DTT；未知能力页仍拒绝。
- 把可执行目标、关联域快照和恢复步骤归入 powerctl/mode；按已声明的 page7 能力读取 ASPM 双向包15、电源计划包29、分模式风扇保存槽，先全量预检再写入。
- Windows 电源 API 单独归入 HAL，测试必须显式注入内存实现。ASPM、活动计划和风扇的首次基线先于主模式命令，失败按依赖恢复并保留可重试状态。
- 自动风扇且偏移归零时不重新武装监护，写入确认后解除已有监护；避免模式联动先恢复自动、随后零偏移又启动监护。
- 双向维护 page4[70..78] 的分模式风扇模式与偏移；全速按原厂记忆为自动，静音增强使用安静槽。固件曲线、RPM、模式、偏移共同使用该契约；模式快照保留全部八个槽。
- 两点曲线恢复默认前捕获完整基线，重置后从原厂包读取默认曲线并同步保存值；失败恢复原曲线与字段，不再留下不可撤销的默认操作或过期保存值覆盖新默认。
- 控制台预览包含联动动作，确认绑定已预览目标；档案、自启和托盘继续复用相同执行链，显式档案参数在模式默认策略之后覆盖。
- 手动档案应用同样绑定预览时的模式联动目标；JSON 档案继续保存模式语义，启动应用使用当时原厂策略。离线时仍可查看/导出文件，但缺少模式联动预览时不能手动应用该模式。
- 控制台由 app 层分域维护刷新代次，变更后只通知其他域；各域保留未提交草稿，自己的写入错误不被统一刷新清除。
- 2026-11-13 预设入口调整：控制台顶部直接展示省电、安静、娱乐、性能四种原厂预设及用途；静音增强仅在能力声明时补充。模式编号与名称继续由 api/power.ts 拥有，不创建虚构 PL/电压或风扇参数的第二套预设。卡片只改草稿，明确区分当前使用、待应用、读取中和固件不支持；确认绑定原厂联动目标，保留恢复初始及跨域刷新。读取失败清除可执行快照，重试成功前禁止写入；个人参数仍由性能档案保存。
- 独显关断、屏幕刷新率和逐键协议不纳入该模式事务。后续已从本机安装载荷找回cc30wk并核对ACPI控制节点与cmd4/sub34协议；其接入与档案联动另行实现，不把模式事务完成当作这些功能完成。
- 2026-11-16恢复预览目检发现：Collapsible内容默认靠起始边对齐，共享OemModeRestore没有宽度，宽屏缩成窄条，窄屏又被FactList的内框和行线挤压。恢复面板明确填满父内容区，按模式与性能、风扇、Windows计划分组；FactList唯一原语增加tiles布局，标签和值上下排列、按可用宽度分列，移除该布局的内框与行分隔线。其他事实行保持现有布局，不全局改折叠容器对齐。

2026-11-19 娱乐与性能模式共用「寂静模式」开关。娱乐模式按原厂能力读取独立的 page4[9]，确认后发送寂静或自动档并保留自动风扇记忆。性能模式复用性能槽和 Turbo 互斥。预览修订绑定当前模式与勾选状态，换档后重新读取对应草稿。只在调整娱乐偏好时登记该保存位的首次基线，应用失败和用户恢复都回读原值。

2026-11-19 娱乐和性能都可应用各自的寂静模式。已保存开启但尚未生效时可重新应用。若当前为自定义固件风扇曲线，应用或关闭寂静模式前显示要切换的固件档位，确认绑定已读修订和自定义来源。复用模式与风扇的首次快照及失败补偿，软件曲线仍按现有交接规则处理。性能选项按各字段的固件能力显示，Turbo 风扇和动态调校 DTT 的有效模式以已核验的命令为准，性能模式无风扇记忆槽时提示切换将重置寂静设置。

2026-11-19 复核本机 INSYDE ADL DSDT 与 DptfTabl。两个 OEM UI 标记都为 0，实际 SCMD 0x79/sub0x19 由 PSF4 bit2 门控，低六位分别保留性能 2 或娱乐 3，并独立解析 Turbo 与 DTT。已核对固件以实时命令门控开放两档选项。DTT 另验当前已启动的 IETM 设备，核对表的 _STA 只有 DPTF = 1 且 IN34 = 1 才返回可用。本机只读确认命令门控和 IETM 均有效。娱乐档从固件自定义曲线开启 Turbo 时先预览自动风扇目标，再执行应用，可恢复原曲线。

## 影响与验证

- 2026-11-20 芯片版通过打包前端的 12 项模拟 IPC 回归。覆盖四档渐变、扫光与抬升、静音增强、键盘与触屏、两种减少动效、写入互斥和回读失败。900 × 560 窗口中芯片压缩至 148 CSS 像素，性能选项标题保留在视口内。扫光截帧见 `target/ui-check/preset-chip-scan.png`。配置档案的两条原厂冲突用例在改造前对照构建也卡在相同按钮定位。验证使用隔离浏览器，没有执行真机切档。

- 2026-11-19 扁平插画通过 29 项打包前端的隔离 Edge 模拟 IPC 检查，覆盖图片加载、浅深主题与宽窄布局、键盘切换、静音增强和供电与热键缩略图复用。前端构建、控件检查和 Rust 工作区检查通过。宽窄浅深截图位于 `target/ui-check/preset-artwork-*.png`。

- 2026-11-19 本机 `FnKey.exe` 7.101.0.0 的包身份、映像、会话及隐藏主窗已只读核验。隔离 WinForms 验证通过，覆盖正常退出清理、拒绝退出、同名窗口保护、未注册映像、进程替换及额外控制进程拦截。31 项模式与布局页面用例通过，其中 6 项复核确认、取消、目标变化、超时和离页。证据在 `target/fnkey-exit-proof/`。本机原厂进程和真实硬件模式未执行切换。

- 2026-11-18 预设直接切换通过 47 项隔离页面检查及 12 项模式联动与恢复模拟。覆盖鼠标和键盘激活、连续点击保护、设备回读、过期目标拒绝、固件与 GPU 事务、插电偏好、初始恢复及校准互斥。前端构建、控件检查和离线 Rust 检查通过。

- 2026-11-18 模式选项扩展通过 128 项隔离模式测试、1 项隔离风扇监护进程测试、3 项风扇记忆测试、19 项档案性能选项测试及 34 项模拟 IPC 页面检查。覆盖娱乐偏好保存、DTT 激活资格、寂静与 Turbo 互斥、无自定义曲线设备、草稿回读、写入失败补偿及恢复。前端构建、控件检查和工作区 Rust 检查通过。监护夹具单独从源码编译。

- 2026-11-18 简化线稿通过 13 项隔离 Edge 检查，覆盖宽窄屏与浅深主题、卡片内图形居中和状态文字完整显示、键盘焦点及一次应用，以及静音增强、供电和热键缩略图的复用。前端构建、控件检查及工作区 Rust 检查通过。

2026-11-13 的本地生图方案为历史视觉版本，素材及资源引用已随 SVG 方案移除。

集成 Surface / Radio 原语时，选中卡片的悬停边框继续显示选中态，卡片右上角为选择标记留白。装饰图形不参与焦点与状态宣读。

- 模式域14项、档案34项、风扇域19项聚焦检查通过；覆盖模式4、旧/扩展能力页、分模式风扇/ASPM/计划目标、阶段失败恢复和默认曲线恢复。系统调用由显式内存实现替代。
- 实际 React 页面模拟 IPC 的模式/档案联动22项通过；控制台63项、软件曲线30项、RPM30项、即时档案22项回归通过。联动变更拒绝旧预览，写入完成刷新其他域且不丢草稿。
- npm run build 与 cargo check --offline --workspace 通过；不运行真实应用或系统电源写 API。
- 顶部预设入口的构建、边界检查与笔记校验通过；构建产物在隔离 Chromium / 模拟 IPC 下的 38 项聚焦检查通过，覆盖四种预设、扩展模式能力、二次确认、完整联动目标、软件曲线退出提示、写入互斥、恢复、跨页草稿、失败重试、迟到读取丢弃、键盘导航和浅深/窄屏。截图与结果留在 target/preset-modes-ui/；不加载真实硬件提供者。
- 历史生图版本通过 47 项隔离 Chromium 模拟 IPC 检查，覆盖图片加载、固有尺寸、装饰隔离、静音增强复用及宽窄深浅布局。结果与页面截图保存在 `target/preset-modes-ui/`。
- 2026-11-16共享恢复布局：前端构建、26项全局控件约束、11项模式偏好/性能选项页面检查通过；宽窄深浅增加面板填充宽度、语义标题和无行线断言，1440浅色及360深色截图已目检。修复宽屏左侧窄条和嵌套内框挤压标签，未更改硬件动作或确认修订。

## 风险

硬件和 Windows 电源策略的实际效果未验证，本轮不调用真实硬件或系统写 API。原厂其他伴随功能的能力与传输必须逐项核对，不能把模式4简单别名为模式0后宣称完成。

## 备选方案

- 继续只发主命令：不满足原厂联动和用户的代码完成要求。
- 依赖原厂 UI 进程代执行：增加隐藏依赖，且无法统一预检与恢复，拒绝。
