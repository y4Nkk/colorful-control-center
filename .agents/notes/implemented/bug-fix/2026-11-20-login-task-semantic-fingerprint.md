# Agent Note: 登录任务指纹改为语义比对

Status: implemented

## 问题

- 2026-11-20 真机注册登录自启后必现「同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖」并整体回滚（用户截图确认，设置页自启项回落「状态未知」并要求重读）。
- 实证（本机 schtasks /create /xml 提交→读回逐元素比对）：任务计划程序存储时会重写任务 XML——`LogonTrigger/UserId` 的 SID 被规范化为 `机器\用户`；等于架构默认值的元素被整体丢弃（Settings 的 Enabled、AllowStartOnDemand、Hidden、Priority、RunOnlyIfIdle、RunOnlyIfNetworkAvailable、WakeToRun，`RunLevel=LeastPrivilege`，`LogonTrigger/Enabled`）；RegistrationInfo 的 URI 改写为实际注册路径并补写 Date。
- `model.rs task_level` 用整树结构指纹与模板严格相等比对，与存储形态永不一致：`transaction::write` 写入后的 `io.state()` 复核必然失败并回滚任务；`get_autostart` 对任何已注册同名任务同样报错。旧测试只做模板自往返（不经真实任务服务），拦不住。

## 决策

- 指纹唯一所有者仍是 `model.rs task_level`，从结构相等改为语义相等：
  - 动作：唯一 Exec，Command 与 Arguments（`--autostart`）逐字一致；额外动作/元素拒绝。
  - 触发器：唯一 LogonTrigger，UserId 等于当前用户 SID 或 SamCompatible 名（任务服务会把 SID 规范化为 `机器\用户`）。
  - 主体：唯一 Principal（`id=Owner`、Actions `Context=Owner` 保留），UserId 同上双形态，LogonType=InteractiveToken；RunLevel 缺省或 LeastPrivilege=Standard，HighestAvailable=Administrator。
  - 设置：逐项接受「等于模板值」或「省略且省略即架构默认值等于模板值」；架构默认值与模板不同的项（AllowHardTerminate=false、StartWhenAvailable=true、ExecutionTimeLimit=PT0S）必须显式存在。
  - RegistrationInfo 不参与指纹：系统补写 Date、改写 URI，纯元数据不影响执行语义。
  - 未知元素一律拒绝。偏差方向失败安全（拒绝并回滚），不会把外来任务误认成本程序的。
- 用户身份第二表示由 `native::user_name()` 经 GetUserNameExW(NameSamCompatible) 从进程令牌取得；`Native` 持有 user+user_name，state()/task()/validate/worker 全链路接线。`xml()` 模板不变，仍是唯一提交形态与 State 规范形态；`State.task` 恒为模板重建值，事务等值复核语义保持。
- 删除 Tree/tree 结构比对（本次改动孤儿）；旧三参签名无兼容残留。Cargo.toml 增加 `Win32_Security_Authentication_Identity` feature（GetUserNameExW 所在模块）。

## 验证

- 新增真机规范化夹具测试：标准（无 RunLevel、触发器为 `机器\用户`）与管理员形态、SID 与 SamCompatible 混合形态均正确分级；规范化形态的篡改全部拒绝（改参数、两种用户表示换人、删除必设项 AllowHardTerminate/ExecutionTimeLimit/StartWhenAvailable、改 StartWhenAvailable/MultipleInstancesPolicy 值、未知设置元素、额外触发器/动作、命名空间替换）。
- 模板往返与既有篡改用例更新签名后全部保留。
- 见提交时的 `cargo test`（login 模块过滤）、`cargo check`、`npm run build`、`npm run verify-notes` 结果。
- 未在真机端到端注册：夹具为 2026-11-20 本机 schtasks 实测捕获的存储形态；COM RegisterTask 与 schtasks 走同一任务服务规范化路径，且接受「省略」与「双用户形态」覆盖两侧可能的差异。真实登录任务注册与下次开机自启仍待真机验收。

## 影响

- 只改登录自启读/写指纹判定与用户身份第二表示接线；事务补偿、恢复记录、NSIS 卸载清理、提权会话流程不动。
- 机器改名后 SamCompatible 变化会导致存量任务触发器用户不匹配而拒绝（失败安全），重新保存自启即修复；SID 形态主体不受影响。

## 备选方案

- 改用 ITaskDefinition COM 属性读取代替 XML 比对：绕开规范化但引入大量 unsafe COM 遍历且无法纯逻辑测试，拒绝。
- 用环境变量 USERDOMAIN\USERNAME 当第二身份表示：可被进程环境伪造，不如令牌侧 GetUserNameExW，拒绝。
- 保留结构比对、注册后跳过复核：破坏「写入后系统前态必须等于目标」的事务不变量，拒绝。
