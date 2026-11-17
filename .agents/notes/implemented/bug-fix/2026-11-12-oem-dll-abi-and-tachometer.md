# Agent Note: 修正本机 DCHU DLL 返回值与测速计数器

Status: implemented

## 问题

此前传输封装把写入成功统一假定为 0，并把 cmd12 的测速计数器直接显示为 RPM。此次核对本机原厂 DLL 原生反汇编和 FanSpeedSetting 的最终 UI 换算，发现这两个假设错误；领域内存模拟绕过了 FFI 层，没有覆盖它们。

## 决定

- 唯一 ABI 事实为本机 DCHU/InsydeDCHU.dll，SHA256 D385DEF14C5AA6CBB8EB60FD34D0EFEB92EAF39E875AB28DE05FE0FDAA7CAB9C。GetBuffer 与 SetDataEx 成功返回命令号，ReadAppSettings 成功返回 4096，WriteAppSettings 成功返回 1；失败 0 不得伪造成成功。SetData 根据固件整数或缓冲结果返回状态，当前实际使用命令均返回命令号。
- GetBuffer 与 SetDataEx 的缓冲复制会在有效负载之后额外复制首字节。FFI 必须为其保留完整输出空间，不能只分配 256B；输出在内部按 ACPI 0x40C 返回容器的上界留空间，领域仍只消费 256B。双向调用返回命令号也可能没有填充输出，须检测该情况。
- AppSettings 的存储总长为 4096，页号只能 0–15，单页范围仍不跨 256 字节。读取只复制请求长度。
- GetInteger 返回负载本身而非命令号；输出初始化哨兵区分失败的 0 与有效的整数 0。回声仍只用于诊断，不用作传感或能力依据。
- 按原厂 Page_system_monitor.UpdateUI_CPUFan/UpdateUI_GPUFan 的公式转换：counter=0→0，否则 round_even(60/(5.565217391304348E-05×counter)×2)。不改变已有 RPM 表中“最大 RPM”字段的含义。
- 探测 CLI 与桌面共用传输和换算事实；用 extern system 内存桩验证真实 FFI 包装，不调用原厂 DLL 或驱动。

## 影响与验证

- 原生 HAL 聚焦 31 项通过（含 ABI 7 项、传感解码 4 项）；探测 CLI 直接复用该 FFI 的 7 项内存桩检查通过。历史原始计数器 474 换算为 4549 RPM 的核心测试通过。
- npm run build、cargo check --workspace、probe-cli 检查、15 篇 verify-notes 及 diff --check 通过。NSIS 安装包重编译完成，7z t 完整性校验通过（1843394B，SHA256 a56093f048e55802d817f4816d2d440ba3b79a333c818993a87be9f14c8afa89）；未安装或启动。
- 不加载原厂 DLL、不执行真实硬件读写。DSDT 只读复制和反编译证据不得描述成风扇或电压实验。原生反汇编与目标 DSDT 节选在本会话隔离研究目录中保留；事实归属 docs/oem-dchu-contract.md。

## 风险

本次事实绑定到审查过的本机二进制与固件；没有证明其他 DLL 版本采用相同约定。GetBuffer/SetDataEx 不返回有效负载长度，现有 256B 包要求仍须由协议与领域校验保证。

## 备选方案

- 同时接受 0 和命令号：把本机明确的失败也当成功，不采用。
- 直接照搬其他 DLL 版本的返回约定：版本哈希不同，不采用。
- 只修界面读数而不修 FFI：硬件动作仍会误判失败，不采用。
