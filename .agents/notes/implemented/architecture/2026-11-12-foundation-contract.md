# Agent Note: 全局底座技术契约（极简自研 / 中文单语 / 笔记完整版）

Status: implemented

## 问题

动工功能代码前需要确定前端底座形态。两个参考项目模式差异大：agent-switchboard 是零依赖极简自研（无路由库、无状态库、无 CSS 框架），spiralcoder 是生态组合（Tailwind 4 + zustand + react-router + shadcn）。底座一旦选定，后续所有模块的接入方式随之固定，返工成本高。同时需要确定：界面文案语言（单语还是双语骨架）、架构决策用什么机制管理。

## 决定

用户 2026-11-12 拍板，三项决定：

1. **前端底座采用 agent-switchboard 同款极简自研**：无路由库（`navigation.ts` 注册表 + `AppWorkspace` keep-alive）、无状态库（模块级单例 store + 命令式 API，app 层唯一挂载 Host）、无 CSS 框架（`tokens.css` 唯一令牌所有者）。零新增运行时依赖。
2. **界面文案中文单语**：不建 i18n 体系，直接写中文；文案不做深层字符串拼接，保留未来机械抽取成 key 的路径。
3. **决策笔记采用完整版**：三态目录（proposed / implemented / rejected + archived）+ `verify-notes` 校验脚本挂构建链，格式契约与 agent-switchboard 对齐。

## 影响

- 所有页面模块按 AGENTS.md「全局化接入契约」三个 checklist 接入；IPC 边界、令牌所有权、页面接线由 `scripts/check-boundaries.mjs` 在构建期强制。
- 全局通知、主题、IPC 边界的唯一所有者在 DESIGN.md「组件所有权」表登记；新增全局原语必须先登记再实现。
- 中文单语意味着面向用户字符串直接写中文，无 MessageKey 类型；抽取成本通过「文案不深拼接」约束保持可控。
- verify-notes 红灯必须先修再交付；笔记格式（首节 ## 问题、Status 行、备选方案节）从此强制。

## 备选方案

- **生态组合（Tailwind 4 + zustand + react-router + shadcn/ui）**：写页面更快、生态成熟；但 UI 参考与设计语言来自 agent-switchboard，shadcn 默认视觉与其「组件所有权 + 唯一原语」契约冲突，且一次性引入四类重依赖，底座期无对应收益。放弃。
- **双语骨架（编译期 MessageKey + zh/en 文案目录）**：起步即支持双语言；但个人硬件工具无近期双语需求，类型化文案体系的维护成本立即发生、收益为零。保留中文单语 + 机械抽取的演进路径，放弃起步引入。
- **轻量版决策笔记（只建目录不做校验脚本）**：搭建更快；但参考项目实践（spiralcoder 375 篇笔记全靠脚本约束格式）表明校验脚本是规则得以执行的关键，且脚本是一次性约百行的成本。选择完整版。
