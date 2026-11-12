# 真机验收清单（尽量少占用你的时间）

> 开发遵循「避免调试机器」：所有需要真机/管理员的验证已批量化为两个入口。
> 完成后把结论回填到本文与 oem-dchu-contract.md §11。

## 1. 一键提权验证批（约 2 分钟，一次 UAC）

右键管理员运行（或脚本会自动请求提升）：

```powershell
tools\probe\run-admin-probe.ps1
```

产出 `tools/probe/admin-probe-results.txt`，回答四个问题：

| 问题 | 判读 |
|---|---|
| MSR PL 锁（0x610 bit63） | 未锁 → 运行时 PL 直写可用；锁 → 只走 page6 持久化通道 |
| undervolt 锁（0x150 写入测试） | 未锁 → 运行时降压可用；锁 → page6+XTU 通道（生效时机经 OEM 服务）|
| DCHU 管理员读数 | 两次采样有变化 → 非管理员回显问题确认；仍恒定 → DLL 还有未知初始化，回填契约 §11-0 |
| PawnIO | `ccc-probe status` 应显示已安装 |

## 2. 应用目检（npm run tauri dev，约 5 分钟）

```bash
npm run tauri dev
```

| 页 | 检查点 | 验收 |
|---|---|---|
| 监控仪表盘 | 读数非「读数异常」；5 分钟历史曲线推进；GPU 温度与 NVML 行一致 | 与 HWiNFO/任务管理器对齐（±3°C / ±100MHz）|
| 风扇曲线 | 读出三风扇 T/D 默认值；模式切换（自动/全速/静音）风扇响应 | **先做模式切换**（低风险），曲线写入放到第三步 |
| 风扇曲线 | 应用一条保守曲线（如 T2=55/D2=45、T3=70/D3=60）| 风扇行为按曲线走；OEM CC 显示自定义；重启后保持（page4 持久化）|
| 性能调校 | PL 显示 135/135（WMI18 默认对照）；undervolt 输入 -50mV 持久化应用 | HWiNFO 显示 PL 变化（若 page6 即时生效）；否则重启后验证 |
| 性能档案 | 保存当前 → 修改 PL → 应用档案 → 恢复 | 每步流水线报告成功 |
| 设置 | 主题三档切换即时生效；自启开关注册表生效；关闭窗口后托盘可唤回 | ✓ |

⚠️ 风扇写入验收顺序：模式 → 偏移 → 曲线，每步确认风扇仍在转（转速读数非零）再做下一步。

## 3. 安装包

```bash
npx tauri build
```

产物 `src-tauri/target/release/bundle/nsis/*.exe`。注意：本机无 MSVC Build Tools，
仓库已固定 windows-gnu 工具链；若 NSIS 下载失败，重跑一次或手动放置。

## 4. 已知延后项（完成验收后决定是否推进）

| 项 | 依赖 | 备注 |
|---|---|---|
| 软件闭环自由曲线 + 看门狗 | PawnIO 端口模块（EC 0xE7 直写） | 引擎已就绪（ccc-core），契约 §5 |
| GPU 频率偏移 | NVAPI 绑定 | 决策笔记 2026-11-12-power-dual-channel |
| 键盘 RGB | OEM LEDKB 命令号 | 契约 §11-6 |
| 电源模式固件写入 | OEM native Fn 守护进程命令 | 契约 §11-1；当前仅切 Windows 电源计划 |
