# Agent Note: 弹窗正文两级文字层级与语义图标

Status: implemented

## 问题

信息类弹窗（DetailsDialog 约 48 处）正文全部同一文字色：术语、解释、上下文行、错误、警示只有字重与红黄两色区分。对照 Apple HIG / Primer / Material 3 的对话框规范，缺失「次级文字色」这一层级手段；错误与警示仅靠颜色区分也弱于 WCAG 1.4.1（颜色不得为唯一指示）。

## 决定

弹窗正文建立两级文字层级，装饰仍只存在于共享外壳（dialog.css），内容层保持无图形：

1. 词条解释降次级文字色：`.ccc-parameter-descriptions p` 使用 `--ccc-text-muted`（monitoring.css 唯一所有）。
2. 弹窗内的上下文行、状态行、阻止原因、恢复受限原因改用现有 `ccc-control-caption` 原语（次级色 + sm 字号），不加新 class。
3. 弹窗内错误（`ccc-control-error`）与固件警示（`profile.warnings`，改用 `ccc-control-caption is-warn`）前置 `<Icon name="error"/"warning" size={16}>`；dialog.css 增加「弹窗作用域内 error/is-warn 段落 flex + 图标悬挂」布局规则，页面层不加图标、不受影响。

分类规则（供后续弹窗沿用）：`role="alert"`/错误 → error 图标；`profile.warnings` 固件警示与互斥约束 → is-warn + warning 图标；阻止原因/上下文元数据 → 无图标 caption；说明性正文与确认弹窗描述保持正文色不动。

范围界定：页面层的错误与警示保持原样（仅文字色）；`FactList` 键值行的 value 保持正文色——label 已是 muted，value 是数据本体，降级会让弹窗失去主强调层（Primer subtitle 对应的是上下文行而非数据值）；`ParameterDescriptions` 的 strong 术语保持 `--ccc-text` + 500 字重。

涉及 25 个组件文件（OemPowerMode、KeyboardLighting、KeymapSettings、GpuTuning、BatterySettings、FanPanel、LightBarSettings、PowerPanel、OemPerformanceOptions、MicrophoneSettings、PlatformPower、DisplaySettings、GpuPowerSettings、BatteryCalibrationSettings、ModeCycleFacts、OemModeGpuEffects、GpuModeFacts、SupplyModeSettings、SupplyModeCurrent、AutomaticRefreshSettings、EnergyPolicySettings、LanPolicy、PowerProfilePolicy、TouchpadSettings、OemConflictHost，及 keymap/MacroRecorder、4 个 HotkeyEventPreviews 子 Facts 组件）。

## 影响与验证

DESIGN.md「组件所有权」Dialog 行同步登记两级文字层级与图标规则。图标仍以 `src/components/Icon.tsx` 为唯一字形单元，CSS 不内联 SVG。已核对 *Facts、SupplyModeCurrent/Recovery、OemModeEffects/Effects Gpu 链路均只渲染在弹窗（编辑器或嵌套 DetailsDialog）内，页面渲染路径未引入图标。前端构建（tsc + 边界检查 + vite build）与 `npm run verify-notes` 通过。未涉及硬件写入。

## 风险

`ccc-control-caption` 自带 sm 字号，上下文行随之变小是预期（Primer subtitle 同为小号低对比）；若某处 caption 语义被误用于正文将导致偏小偏灰，靠分类规则约束。警告文案与「误用 muted 呈现错误」是主要回归点，后续新增弹窗按分类规则走。

## 备选方案

- CSS `::before` + mask 内联 SVG 图标：零调用点改动但使字形数据在 Icon.tsx 之外出现第二份，违背「Icon.tsx 界面图标唯一边界」，拒绝。
- 弹窗整体改非模态 popover（Apple 对补充信息的建议）：推翻 DESIGN.md:32 既定交互且迁移 48 处调用点，暂缓（proposed 未立）。
- 全站（含页面）错误段落统一加图标：超出本次「弹窗内」范围，页面层已有面板标题等上下文，未做。
