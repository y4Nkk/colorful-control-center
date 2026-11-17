# 原厂校准电源计划

- 来源：本机安装载荷 `CLEVOCO.BatteryPackUtility` 3.60.1.0 x64，`CC/batteryUtility.pow`。
- `battery-utility.pow` 保留原始字节，SHA256：`8abcae91fe798db277c953e2a203b9ec4953ba94d08fa19f195ddedeadf50151`。
- `battery-utility.json` 是从该文件的 REGF 单元只读提取的显式 AC/DC 索引清单，用于导入后的回读校验；默认组按 Windows `NO_SUBGROUP_GUID` 表达。
- 应用仅导入到按本次任务标识派生的私有 GUID；原计划 GUID `111b4222-7777-6666-8888-ff5bb260df2e` 不作为写入或删除目标。
- 原始文件与生成清单在编译时嵌入，运行时核对哈希；不依赖机器上的原厂安装目录或原厂计划是否已导入。

离线复核：`python tools/probe/inspect-power-plan.py src-tauri/resources/oem/battery-utility.pow --check-index src-tauri/resources/oem/battery-utility.json`。
