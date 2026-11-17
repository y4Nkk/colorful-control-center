# Agent Note: 应用图标液态玻璃材质化

Status: implemented

## 问题

用户要求参考 spiralcoder 的液态玻璃 logo 与 agent-switchboard 的独立标识设计，使用透明背景的 CCC 五叶风扇和蓝紫粉琥珀光谱，同时实现单源、无兼容分支的完整交付。上一轮资产缺少可信的视觉验收，外部 SVG 不响应应用内减少动效，图标与 favicon 依靠手动同步。

## 决定

2026-11-17 用户最新明确要求透明底，避免底部圆形环状存在，并再次要求对照 agent-switchboard、spiralcoder。此要求取代早先圆盘方案：移除圆形底盘、外环与环状反光，让五叶转子本身承载玻璃材质，叶间负空间完全透明。材质参考 `F:/projects/spiralcoder/web/public/logo.svg`，其有机轮廓不复制；CCC 保留自己的风扇形状及左下蓝至右上琥珀的色彩方向。

- `src-tauri/icons/icon.svg` 唯一拥有几何、光谱、材料及内部流彩动画。统一左上光源，亮边、轻微内缘和局部镜面反光落在扇叶内部，不添加圆盘、外圆环或底座；轮毂是转子组成部分，不绘制装饰圆环。静态基准必须独立成立。
- `src/components/BrandLogo.tsx` 读取同一源，隔离 SVG 实例 ID，统一应用内装饰语义；顶栏、关于与设置预览接入，不维护第二份绘图。动效响应既有 `data-motion` 和系统偏好，不新增偏好状态。
- `scripts/generate-icons.mjs` 是生成与检查唯一入口：调用已安装 Tauri CLI 在自有临时目录生成，成功后同步已有桌面图标和 favicon，构建检查过期或缺失资产。删除手动同步作为正式流程的说明，不输出移动端图标。
- `DESIGN.md` 登记 logo 的来源、渲染与生成所有权，局部品牌材料不扩展到应用外框。
- 撤回上一轮“resvg 普遍丢弃 opacity/fill-opacity”的结论：原判断来自未被模型读取的图片，既有源码证据也不支持该结论。光照滤镜是否可用以实际复现和像素检测为准，不围绕未经证实的限制建立契约。

## 影响

本轮透明版实施与验证结果（上一轮圆盘版的目检通过、浏览器与栅格等价等表述全部撤回）：

- **透明底像素验证**：`icon.png` 512×512 可见范围 alpha bbox (48,48,455,464)，四角、底部边缘及轮毂—叶间 12 个采样点 alpha 均为 0；无圆形底盘、外环或底部环状元素（浏览器 DOM 亦确认 0 个独立 circle/ellipse）。透明版初版因 clipPath `<use>` 指向 `<g>` 组导致 resvg 输出整张空图，已改为单条五叶+轮毂复合路径修复；生成器据此新增全透明 `icon.png` 拒绝检查。
- **资产管线**：17 个受管资产（favicon + 16 桌面图标）由 `generate:icons` 单命令生成，`check:icons` 构建时校验；实测 Tauri CLI 的 ICNS 记录顺序不确定，比较按记录规范化后进行，内容变化仍被拒绝（定向测试覆盖顺序变化、内容变化、损坏格式、空图、CLI 失败）。
- **组件与动效**：应用内 17 实例经 `BrandLogo` 渲染，ID 零重复、零悬空引用；`data-motion="reduce"` 与系统 `prefers-reduced-motion` 均使全部流彩动画计算样式为 `none`；动画关键帧 (0,0)→(-56,40)→(0,0) 循环连续，静态帧即完整构图。顶栏 logo 区域拖动命中由 `.ccc-brand-icon { pointer-events: none }` 修复（此前 SVG 内部元素吞掉拖动事件）。
- **验证命令**：`tsc --noEmit`、`check:boundaries`、`cargo check --offline`、`generate:icons` 通过；完整 `build`（含 `check:icons`）与 `verify-notes` 以最终交付前最后一次全量跑为准。视觉证据（源哈希 7711d34d…、多尺寸浅深背景预览与验证 JSON）保存在 `C:/Users/28308/AppData/Local/Temp/ccc-logo-transparent-final-20261117/`。纯视觉资产改动，无硬件写入，不需真机验证；Windows 任务栏/桌面图标实际观感需真机目检确认。

## 备选方案

- 复制 spiralcoder 的有机轮廓：弱化 CCC 风扇身份，不采用。
- 保留手动同步与独立静态设计：产生多个可矛盾的来源，不采用。
- 新增 logo 动效偏好或平行状态：既有 appearance 契约足够，不采用。
- 圆形玻璃盘与外环：被用户最新透明底要求取代，已删除对应几何与材料，不保留变体。
