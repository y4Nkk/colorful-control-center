# EC 差异取证与原厂通道

当前应用通过原厂 InsydeDCHU 命令读取温度、RPM、风扇强度并设置固件曲线。双风扇不需要猜测 GPU EC 地址。主协议和能力判断见 [OEM 契约](../../docs/oem-dchu-contract.md)。

本轮不在机器上执行 EC 探测。`ec-diff.mjs` 只分析已有文件；下面的采集过程供另行安排真机验收时使用，不是应用运行的前置步骤。

## 采集

1. 记录机型、主板、BIOS、OEM 软件版本、采集工具版本、环境温度及负载；先保存 OEM 当前设置。
2. 在同一负载下保存原状态 A 的完整 `ec-probe dump`，只在 OEM UI 改一项得到状态 B，保存完整快照；不得同时运行多个 EC 控制程序。
3. 用 OEM UI 恢复 A，再重复一次 A→B；每次采集对应的 OEM/HWiNFO 温度、RPM、风扇强度及时间戳。风扇控制与 RPM 读数的证据分开记录。
4. 最后通过 OEM UI 恢复原设置。不要执行 `ec-probe write/load`，也不要用上一代 X15 AT22 的表盲写本机。

## 离线比较

```powershell
node tools/probe/ec-diff.mjs before.txt after.txt repeat-before.txt repeat-after.txt
node --test tools/probe/ec-diff.test.mjs
```

输入支持完整 16×16 十六进制表（`00 | …` 或 `00: …`，UTF-8/UTF-16LE、ANSI 颜色）或扩展名为 `.bin` 的 256 字节原始数据。缺行、重复行和非法字节直接报错，不把省略行当作零。

`sameValues` 表示两轮变化前后的值一致；`sameDirection` 只表示数值变化方向一致。两者都不能证明寄存器的硬件含义，温度、计数器和转速也会自然变化。结果始终标记 `hardwareVerified: false`，不会改写 `ec-map.json`。只有结合行为、数值缩放、重复恢复证据后才能更新映射的机型验证状态。

## 开源参考

- [NBFC-Linux 配置库](https://github.com/nbfc-linux/nbfc-linux/tree/main/share/nbfc/configs)：X15 AT22 社区配置仅含一个 CPU 风扇；不能据此推断本机 GPU 写地址。
- [NBFC-Linux ec_probe](https://github.com/nbfc-linux/nbfc-linux/blob/main/src/ec_probe.c)：完整 EC 快照和离线文本格式参考。
- [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor)：PawnIO 监控与 EC 互斥访问参考；本项目优先使用 OEM 通道。
