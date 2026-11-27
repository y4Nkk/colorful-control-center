# Agent Note: Windows 电源计划选择

Status: implemented

## 问题

「电源与档案」页缺少手动选择 Windows 控制面板电源计划的入口。Windows 允许自定义计划和名称。

## 决定

在「电源设置」顶部读取系统计划及活动项。计划名称由 Windows 返回，计划 GUID 区分自定义计划和同名项。选择框默认显示活动项，列表选项上标注当前启用项；选定目标后选择框和预览只显示待应用目标，不另设「当前启用」说明行。应用时在 HAL 写锁内复核计划列表、活动项和电池校准状态，调用系统接口并回读。

计划标识使用小写连字符 GUID。名称由 `PowerReadFriendlyName` 读取，缓冲区不足时按 Windows 返回的字节数扩容并核对 UTF-16。电池校准使用同一个 HAL 通道枚举、导入、核对及删除临时计划。读取和确认时通过 `PowerSettingAccessCheckEx` 检查活动计划写权限。窗口重新聚焦或页面内定期查询发现活动项变化时，刷新关联策略的系统读数并作废已过时选择。 [电源计划管理](https://learn.microsoft.com/en-us/windows/win32/power/managing-power-schemes) [名称读取](https://learn.microsoft.com/en-us/windows/win32/api/powrprof/nf-powrprof-powerreadfriendlyname) [活动计划权限检查](https://learn.microsoft.com/en-us/windows/win32/api/powrprof/nf-powrprof-powersettingaccesscheckex)

计划切换统一从选择列表进入。页面确认后提交，失败时显示系统错误并重新读取活动计划。界面以 Windows 回读的活动项为准。

## 影响与验证

选择 Windows 计划会立即启用其配置。校准接管或尚需恢复时保留读取、暂停切换。原厂模式和供电联动仍可按已配置策略改变活动项。

前端构建、Rust 编译及测试目标检查、55 项相关 Rust 单元测试、7 项校准进程用例、34 项控件检查和 39 项无界面 Chromium 用例通过。原生读取探针核对了本机计划名称和活动项，桌面用户的只读权限查询通过。实机切换、校准导入与删除待验收。

## 备选方案

按固定名称映射三种标准计划无法覆盖用户创建或重命名的计划。
