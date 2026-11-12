# Agent Note: Phase 3 双通道性能调校与 GPU 偏移延后路径

Status: implemented

## 问题

Phase 3 要求 PL1/PL2、undervolt、GPU 频率偏移、性能档案。PL/undervolt 有两条候选通道：OEM page6 持久化（生效时机受 OEM 后台服务/固件控制）与 MSR 运行时直写（需要 PawnIO 驱动 + 管理员一次性安装）。GPU 频率偏移必须走 NVAPI（NVML 无偏移 API），而 Rust 生态没有活跃的 NVAPI 绑定。

## 决定

（2026-11-12）

1. **双通道并存**：`powerctl` 同时实现持久化通道（page6 33/37/41/61/69 + IsSaved=1，与 OEM CPU_OC 完全同款语义，值即"可恢复意图"）与运行时通道（PawnIO IntelMSR 模块：0x610 读改写保留锁位、0x150 OC Mailbox 写后回读校验，锁定自动还原）。UI 两个按钮并列，运行时通道不可用时置灰。
2. **钳位集中**：PL 35–157W、核心偏移 −150…+50mV、GT −150…0mV，全部在 powerctl 入口强制；undervolt 默认值以 WMI18 出厂值为基准显示。
3. **性能档案** = JSON 文件（app_data_dir/profiles），应用流水线顺序：曲线(cmd14)→模式(cmd121)→PL(page6)→电压(page6)，每步独立报告、失败不中断。运行时 MSR 通道不进档案（档案=可恢复意图）。
4. **GPU 频率偏移延后**：等 NVAPI 绑定方案落地（见备选方案）；page6 offset 69 的 iGPU GT 电压偏移已支持。

## 影响

- undervolt 即时可用性取决于 PawnIO 安装（tools/probe/run-admin-probe.ps1 一次完成）与 0x150 锁状态（uv-test --execute 判定）；锁定时持久化通道仍可用（生效时机经 OEM 服务）。
- 档案应用只走持久化语义，重启后行为可预期；"一键满血"若需即时生效，后续可在档案应用后追加运行时通道调用。
- 现有 UI（Performance/Profiles 页）与命令（apply_pl_persist/runtime 等 6 个 + profiles 5 个）已接入注册表。

## 备选方案

- **仅持久化通道**：零驱动依赖；但用户无法即时验证效果（生效时机黑盒），undervolt 调参体验差。补运行时通道。
- **仅运行时通道**：即时生效；但 page6 不写则重启丢失，且强依赖 PawnIO。双通道互补。
- **GPU 偏移现在做 NVAPI FFI**：nvapi.dll 仅导出 QueryInterface，接口发现表需要维护完整 ID 映射（nvapi-sys 的生成代码模式），工作量与 GPU 频率偏移收益不匹配（vBIOS 也常锁功耗）。延后：引入 `nvapi-sys` 生成绑定或参考 LibreHardwareMonitor 的 NvAPI 实现（GPL）单独开 Phase。
