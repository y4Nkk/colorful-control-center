# Colorful Control Center 协作规范

> 工作路由、接入契约与验证约束的唯一所有者。设计契约见 `DESIGN.md`，
> 硬件架构见 `docs/design.md`。本文件与用户全局 AGENTS.md 叠加生效。

## 项目定位

- 七彩虹游戏本（蓝天/联宝模具）的本地硬件控制台：监控传感器、自定义风扇曲线、调节功耗墙/偏移电压/GPU 频率、打包性能档案。
- 不新增云同步、遥测、账户体系、多语言框架、主题商店；硬件写入必须有预览/确认/恢复语义（安全边界见 `docs/design.md` §4）。
- 产品与设计事实分别由 `README.md` 与 `DESIGN.md` 拥有；不维护任务流水文档，架构决策进 `.agents/notes/`。

## 工作路由与契约

| 位置 | 职责 |
|---|---|
| `src/styles/` | 设计令牌（tokens.css 唯一定义处）+ reset + 领域样式（base.css 聚合） |
| `src/api/` | IPC 唯一边界：client.ts 唯一 `invoke()`，`api/<域>.ts` 类型化封装 |
| `src/components/` | 全局原语（通知/占位/未来 Button 等单文件平铺）；业务模块组件按子目录 |
| `src/app/` | 应用骨架：AppShell / AppWorkspace / navigation.ts / 各页 Workspace |
| `src/lib/` | 纯函数工具（theme 等） |
| `src-tauri/src/commands/` | 命令薄层：参数校验与调度，不沉淀业务状态 |
| `src-tauri/src/command_registry.rs` | 全部命令唯一注册清单 |
| `src-tauri/src/<domain>/` | 硬件领域逻辑（hal / fan / power…，出现时创建） |
| `tools/`、`scripts/` | 探测工具与可执行规范脚本 |
| `docs/` | 硬件架构文档 |

- UI 只能请求类型化命令；`invoke` 与 `@tauri-apps` import 不得离开 `src/api/`（`check:boundaries` 强制）。
- UI 改动先读 `DESIGN.md` 相关节与 `src/styles/`；硬件行为改动先读 `docs/design.md` 并检查受影响 HAL 契约。

## 全局化接入契约

全局能力先在 app 层定义，模块位置只接入。三个标准操作：

**新增页面**
1. `src/app/navigation.ts`：`PageId` 联合类型 + `PAGES` 注册（中文标题直写）
2. `src/app/<Page>Workspace.tsx`：创建 Workspace（接收 `active` prop）
3. `src/app/AppWorkspace.tsx`：新增 keep-alive 分支（visited + hidden）
4. `DESIGN.md`「页面结构」节登记该页职责
5. `npm run build` 通过（`check:boundaries` 校验接线一致）

**新增 IPC 命令**
1. `src-tauri/src/commands/<域>.rs`：实现 `#[tauri::command]`（薄委托，逻辑放领域模块）
2. `src-tauri/src/commands/mod.rs`：登记子模块契约
3. `src-tauri/src/command_registry.rs`：注册清单加入命令全路径
4. `src/api/<域>.ts`：类型化封装 + `client.ts` re-export
5. `npm run build` + `cargo check` 通过

**新增全局组件**
1. `DESIGN.md`「组件所有权」表登记（先登记再实现）
2. `src/components/` 实现单例 store / Host / 原语
3. app 层（`App.tsx` / `AppShell`）唯一挂载
4. 模块位置只 import 调用，禁止模块自建平行实现

## 决策笔记（Agent Notes）

- 非平凡改动（行为、架构、跨文件契约、流程与工具链、落盘/硬件写入语义）**前**，在 `.agents/notes/` 写或更新笔记；机械性小改（样式微调、格式化、不改行为补丁）直接提交代码。
- 结构：`{lifecycle}/{class}/yyyy-mm-dd-topic.md`；lifecycle ∈ proposed/implemented/rejected；class ∈ feature/bug-fix/simplification/architecture/process/testing；被取代的笔记移入 `archived/` 并注明被谁取代。
- 写之前先检索同主题旧笔记：有归属就地更新事实；新想法放 `proposed/`，落地随代码改动转 `implemented/`。
- 落盘后跑 `npm run verify-notes`，红了先修再交。

## 验证

- `npm run build`（tsc + 边界检查 + vite build）与 `cargo check` 是交付底线。
- 硬件写入相关改动必须在汇报中明确标注风险与真机验证状态；只读探测工具改动不要求真机验证。
- 除非用户明确要求，不运行完整测试套件；按最小相关范围验证。
