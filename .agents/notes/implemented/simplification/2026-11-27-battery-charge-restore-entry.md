# Agent Note: 移除充电保护的初始恢复入口

Status: implemented

## 问题

「电源与档案 → 电池与充电」卡片在修改过充电保护后出现「恢复充电保护」按钮，用于回到本次运行中首次修改前的范围。用户要求移除该按钮，并沿用 2026-11-24 设备配置页移除恢复入口时确定的标准：无兼容、无过渡、无技术债，不能只藏起按钮而把后端恢复路径留成无人调用的代码，也不能留下删掉按钮后无处可出的状态。

## 决定

2026-11-27 卡片不再渲染恢复入口，只保留限制开关和起止阈值。只服务于初始恢复的路径在所有层一并删除：

- IPC：`restore_battery_charge` 命令，连同注册表项和 `src/api/battery.ts` 的 `restoreBatteryCharge` 封装。
- 领域：`battery::Controller` 进程内存里的首次基线、`Profile.canRestore`、`battery::restore` 与 `charge::Plan::restore`。基线不落盘，没有记录格式或版本需要迁移。
- 写入门禁：`charge::Plan::prepare` 与 `Plan::profile` 中「当前候选须与首次基线一致」的检查。它只为保住恢复目标而存在。单项请求绑定含候选的修订，档案目标在 `Scope::prepare` 里单独核对自身候选，这两处原有约束保留。
- 测试：直接断言基线与恢复的用例删除或改写；页面用例改为用模块自身控件切回，并断言卡片不出现恢复按钮；模拟 IPC 删除 `restore_battery_charge` 处理、`batteryInitial` 和电池 `canRestore`。
- 文档：`docs/battery-contract.md`、`docs/design.md` 和更新日志同步。

保留的部分：`charge::Plan::execute` 与 `rollback` 的本次前态补偿、档案应用的逐域回滚、节能计划自己的 `restore_energy_policy` 与持久初始快照、校准的 `restore_battery_calibration`。它们使用各自操作的前态或记录，与被删的首次基线无关。

`battery::Scope` 同时承担档案事务期间的写互斥，予以保留：锁对象由首次基线换成专用的 `mutation: Mutex<()>`，与键盘、按键映射和独显电源域的做法一致。先取 Scope 再取 HAL 的顺序不变。

恢复入口不是任何状态的唯一出口：按钮的禁用条件 `busy || !canWrite || !canRestore` 比开关和阈值的 `busy || !canWrite` 更严，按钮可点时自身控件必然可用。删除后改用不依赖恢复按钮的规则：

1. **补偿失败**：写入失败且回退也失败时，固件停在目标值或写入前值，错误照旧显示并重新读取。重新选择所需值即可，不再需要经恢复按钮重试。
2. **运行中候选变化**：原实现在首次写入后，若固件返回的候选与首次基线不同，会同时拒绝新的写入和恢复，直到重启。候选检查随基线删除后，重新读取即可按当前候选继续设置。

## 影响与验证

- 充电保护只能用限制开关和起止阈值切回。固件原始关闭态的零阈值（0/0）不再能经卡片控件精确还原：关闭时保留最后一次起止值，效果同为充至满电。档案回放仍可精确重放 0/0 的关闭目标。
- 后端删除的都是恢复与只读展示路径，写入与补偿路径未改；候选门禁是唯一的行为变更。
- 早前笔记中关于充电首次基线、零阈值精确恢复和「单项/档案共用基线」的描述由本条取代，见 `implemented/feature/2026-11-13-oem-battery-charge.md` 和 `implemented/feature/2026-11-14-complete-device-profiles.md`。
- 验证（2026-11-27，全部为隔离验证，未操作真实硬件）：`cargo check -p colorful-control-center --locked --lib --tests` 通过，测试构建的 60 条警告均位于本次未改动的文件。`cargo test -p colorful-control-center --locked --lib -- battery profiles:: command_registry` 560 项通过、0 失败、2 项忽略，其中名称含 battery 的 190 项。`npm run build` 通过。
- UI 检查按 `battery energy calibration mode-preference` 过滤共 78 项：76 项通过，2 项失败均与本次改动无关。`profile-battery.spec.mjs:107` 失败于另一会话进行中的内置档案功能（`read_builtin_profiles` 当时未在模拟 IPC 声明），对方补齐后重跑通过。`battery-lifecycle.spec.mjs:10` 在纯 `HEAD` 副本和去掉本次前端改动的工作区副本中以相同断言失败，属既有问题：点击当前页面的侧栏按钮会保留「配置档案」分区，用例却期待回到「电源设置」。未替他人修改。
- 未做真机验证：按钮消失后的卡片布局只在模拟 IPC 上截图目检；补偿失败后重新提交、运行中候选变化后的写入只有内存传输检查。

## 备选方案

- 只删界面、保留命令与封装：命令没有调用方，属于被要求清除的技术债，未采用。
- 保留首次基线但不再展示按钮：留下无人读取的状态和只为它存在的候选门禁，未采用。
- 保留候选门禁：它会在运行中候选变化后把模块锁到重启，而它保护的恢复目标已不存在，未采用。
