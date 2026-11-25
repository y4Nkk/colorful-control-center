# Agent Note: 后台读取、实时渲染与档案列表优化

Status: implemented

## 问题

窗口在非监控界面时仍执行完整硬件显示采样并广播。托盘的温度提示与控制菜单共享刷新周期。前端的实时读数驱动工作区级更新。内存容量轮询同时查询硬件规格和磁盘卷。逐键灯效在无按键和过渡时仍定时唤醒。档案列表重复解析和校验内容一致的文件。

## 决定

- 主窗口可见且停留在监控页时请求硬件显示会话。唯一的原生采样线程立即采样，再按 1 秒间隔定向发布。托盘悬停信息每 5 秒更新，复用新鲜的显示温度或实读温度。右键菜单在打开时实读硬件和档案，写入后回读更新悬停状态。软件风扇、显示试用、电池校准的监护独立调度。
- 状态、概览图表与风扇实时读数各自在消费组件订阅，停滞提示由最后一次到达安排一次性超时。温度图和容量卡收到相同输入时复用渲染。内存容量每 5 秒更新，固定卷容量每 30 秒更新，卷身份按 GUID 核对，卷元数据有效期 120 秒。窗口恢复或重新进入概览时立即刷新。成功读取的 CPU 与 SMBIOS 规格在进程内复用。
- 灯效动画由 MCU 执行。主机输出线程等待按键、会话状态变化或左 Win 键的 50 ms 截止点。保留 64 项 FIFO、溢出停机和输出代次检查。
- 档案列表每次实读文件，字节完全一致时复用已校验元数据。缓存限 256 份和 4 MiB 原始缓冲区。列表与保存、删除共用存储锁，相关 IPC 在阻塞线程执行，实际写入和授权继续实读。
- 原有单次硬件快照命令和综合系统概览命令移除。前端与模拟 IPC 同步切换。

参考 [React 外部状态订阅](https://react.dev/reference/react/useSyncExternalStore)、[React memo](https://react.dev/reference/react/memo)、[Tauri 异步命令](https://v2.tauri.app/develop/calling-rust/#async-commands)、[Microsoft 卷 GUID](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumenameforvolumemountpointw)、[Rust park](https://doc.rust-lang.org/std/thread/fn.park.html) 和 [Git 的时间戳竞态说明](https://git-scm.com/docs/racy-git)。

## 影响与验证

- `npm run build`、`cargo check --offline --workspace` 和 77 项 Rust 聚焦测试通过。生产构建的 48 项不同 Edge 用例通过，覆盖切页、隐藏、首个样本超时、重连、停滞、内存与磁盘分频、档案和托盘导航。
- 最终构建概览入口 JavaScript 及静态依赖为 471,664 字节，拆分前为 814,275 字节，减少 42.1%。128 份临时档案的 Debug 暖列表中位耗时从 64.29 ms 降至 37.03 ms。Windows C:、F: 的卷 GUID 和盘符容量接口结果一致。

## 风险

托盘首次菜单耗时、实际后台功耗和真机逐键效果仍待量化。逐键报文、事务锁顺序和补偿在隔离传输中验证。

## 备选方案

全局拉长采样周期会连带影响实时读数。仅信任文件时间戳会漏掉内容已变化的档案。引入并行硬件写入会改变硬件事务顺序。
