# Agent Note: 原厂低电量阈值事件与逐计划恢复

Status: implemented

## 问题

FnKey 7.19.5.0 的 Arg001 209/210 分别调用 `StoreAndSetBatteryLow1Low2` 和 `RestoreBatteryLow1Low2`；原先没有路由。原厂209把活动计划的三个直流阈值暂存到 page1[47..49]，然后设低电量25%、严重电量20%、预留22%；210把缓存值写回并清零。该路径修改 Windows 电源计划，不能与已有充电保护阈值混用。

## 实现

- 静态依据：Pi 会话的 `FnKey/Features.cs` 与 `Form1.cs`；同包 `powerlife.dll` SHA256 `cabd0a2c74b3f1d6eb7be4b3e0e601bd855e9b6694009ba980f6244c141405e2`。`SetLowBatteryLevel` RVA 0x5570、`SetCriticalBatteryLevel` RVA 0x53D0、`SetReservelBatteryLevel` RVA 0x5710 逐次调用 `PowerWriteDCValueIndex` 和 `PowerWriteACValueIndex`，目标为活动计划及三个标准计划，不是只修改当前电池值。三个Windows设置分别为 `8183ba9a-e910-48da-8769-14ae6dc1170a`、`9a66d8d7-4ff7-4ef9-b5a2-5a326ca2a469`、`f3c5027d-cd16-4930-aa6b-90db844a8f00`，电池子组为 `e73a048d-bf27-4f12-9731-8b2076e8891f`；与 Microsoft 的[低电量](https://learn.microsoft.com/en-us/windows-hardware/customize/power-settings/battery-settings-low-battery-threshold)、[严重电量](https://learn.microsoft.com/en-us/windows-hardware/customize/power-settings/battery-settings-critical-battery-threshold)及[预留电量](https://learn.microsoft.com/en-us/windows-hardware/customize/power-settings/battery-settings-reserve-battery-level)契约一致。
- 接入独立授权范围及模块化预览，明确展示所有存在的目标计划、插电/电池的实际数值、三个目标和缓存字段；活动计划与标准计划去重，不创建缺失计划。长期授权绑定设备及能力范围；每次事件预检活动计划、计划集合、页面缓存、权限及校准互斥。
- 用单一持久恢复记录保存每个受影响计划及每个AC/DC字段的完整原值和原厂缓存字节，在任何目标写入前原子落盘；逐字段标记尝试、回读，失败逆序补偿，恢复失败保留可重试记录。209重复事件不覆盖首次基线；210及显式恢复只恢复本应用拥有的字段，保留与本次无关的计划及设置。已有未知原厂缓存或记录损坏时拒绝覆盖，不把三个DC缓存冒充所有计划/供电的原值。
- 原厂`SetBatteryLow1Low2ToEC`声明存在但当前静态输入未发现调用者，不以方法存在为依据自动发送固件阈值；44/45的停止轮询及睡眠/休眠、59/60的低电量亮度和47温度警告需各自核对并独立设计，不由209/210隐式授权。

## 验证范围

原厂计划集合与AC/DC字段、旧缓存、重复事件、计划变化、部分写入/进程中断、原值为0、外部改写及恢复失败有内存与私有临时目录检查；用户可预览、确认、恢复；相关档案、校准、热键及页面回归通过。仅编译、模拟和静态文件分析，不访问真实电源计划、固件或执行睡眠。

## 风险

原版只保存活动计划的三个直流值，却写入多个计划的AC/DC字段；照原样恢复会覆盖其他原值。首次状态或原厂缓存不可确认时不得猜测，也不能自动丢弃恢复记录。操作可能受Windows策略或权限限制，必须回读并报告确切失败。

## 备选方案

- 用原厂三个缓存值回写所有计划：无法精确恢复不同计划和供电状态，拒绝。
- 只改活动计划：与原厂DLL实际写入范围不符，拒绝。

## 决定

按上述单一持久恢复记录与独立热键范围实现209/210；界面分开当前值、百分比目标和逐计划恢复。外部改变被接管字段时展示恢复记录并拒绝覆盖。校准、许可、计划和缓存状态在输出前重查，停止监听与恢复计划分开。

## 影响与验证

14项事务及原生函数指针桩、10项核心事件、23项相关模拟页面通过；覆盖各计划AC/DC不同原值及0、重复事件、许可撤销、计划切换、部分写入、进程中断状态、恢复重试和外部修改。主程序与模拟宿主编译、前端构建、控件和源码限制通过。测试仅用内存、私有临时目录及模拟IPC，没有访问实际电源计划、DCHU或原厂事件。GPU Turbo阶段安装包不包含本功能；其余原厂低电量和音量分支继续核对。
