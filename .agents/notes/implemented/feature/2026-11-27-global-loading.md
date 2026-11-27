# Agent Note: 全局等待与进度原语

Status: implemented

## 问题

等待状态没有唯一所有者。读取中的文案在三十余处手写，共四种句式；`loading` 图标只在侧栏更新入口旋转，按钮里静止；按钮忙碌由调用方各自拼接 `icon="loading"`、`aria-busy` 与 `disabled`；启动页循环进度条与更新弹窗的原生 `<progress>` 是两套实现。这与 DESIGN.md 的「不建平行组件」和「同一概念使用同一控件」相悖。

## 决定

- 新增 `Loading` 原语：旋转的 `loading` 图标加完整说明文字，自带 `status` 语义；嵌入既有常驻状态区时用 `embedded` 省略 role。数据首读、分包加载与动作执行中在所属位置就地呈现，不引入全屏遮罩。
- `Button` 增加 `busy`：锁定入口、写 `aria-busy` 并把图标位换成 loading 图标。调用方不再手工拼接。
- 新增 `Progress` 原语：已知总量按比例填充，未知总量循环。启动视图与更新下载共用，外观、减少动效和强制颜色处理只有一份。
- `loading` 图标的旋转由 `icons.css` 唯一定义，时长取新令牌 `--ccc-motion-spin`；`--ccc-motion-preparation` 更名为 `--ccc-motion-progress`。
- `scripts/ui-contract.mjs` 登记两个原语；原生 `<progress>` 与 `progressbar` 角色归 `Progress`，业务不得再直接使用。
- 读取文案统一为「正在读取X…」，动作统一为「正在X…」，分包保留「正在加载X…」。
- `src/app/WorkspaceLoading.tsx` 删除，页面分包与键盘页首读改用 `Loading`。
- 电池、Windows 电源计划、屏幕及各设备首读用已有读取阶段区分等待和结果。模式与更新保留各自的常驻播报区，由 `Loading` 呈现未完成的阶段。持续采样沿用各图表的缺测状态。

## 影响与验证

`tools/ui-check/loading.spec.mjs` 覆盖首次等待、减少动效、按钮忙碌以及读取失败后重试。启动和更新用例覆盖已知与未知进度、签名校验、取消和错误。控件边界校验把原生进度条、进度角色及全局组件样式限制在各自的定义处。

## 备选方案

- 仅统一文字，不加图形：等待状态缺少运动提示，按钮内也仍无进度表现。
- 骨架屏：每张卡片需要单独的占位布局，且不适用于按钮与分包。
- 全屏遮罩：与 DESIGN.md「忙碌时锁定操作、保留读数」相悖。
