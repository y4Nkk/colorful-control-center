# Agent Note: 原厂186/187电池音频会话

Status: implemented

## 问题

186/187属于独立电池事件链，不能用主扬声器静音、自造提示音或59/60授权替代。研究目录缺少audio10ec.dll不能作为停止实现的理由，实际本机FnKey嵌套AppX包含该文件。原先低电量领域将自然结束与用户初始恢复混用，也会阻止准确复刻。

## 决定

- 播放器停止失败后即使工作线程已结束，仍保留播放所有权；全局停止入口允许在该状态重试，并在没有工作线程时直接调用同一清理。线程结束刷新领域事实，避免前端仅看到监听已停就隐藏仍待清理的循环。应用/恢复/重置使用同一停止所有者，不增加第二个音频停止路径。

- `batteryAlert`作为第25个独立范围；绑定WMI70 bit13、扩展表/客户、面板/计划和PC Beep控件身份，静音值和电量不进入稳定授权。无效或回声整数、未知版本、设备变化及未完成事务拒绝执行，不扩大旧范围许可。
- 186按E_Data bit7/低7位处理绝对0–100%亮度，随后启动程序生成的零样本循环；按能力取消PC Beep静音，并在BATLowControl及EC0x551 bit5/4条件下调用低电量设置，不发59专用data2。越界在首个输出前拒绝。
- `screen_brightness`唯一拥有亮度和缓存记录、领域锁及传输。严格版本2分别记录首次实际前态、最近完成值、本次目标及尝试字段；现存非零缓存也捕获当时实际值，不推测更早的原厂前态。事件部分失败保留首次亮度恢复，不宣称整链原子成功。
- 设置和自然结束目标由同一Rust纯函数生成。重复设置按当时AC更新cache0，仅cache0原为0时调暗；自然结束采用非零当前AC，否则cache0，写AC/DC并只清cache0。用户另行确认才恢复首次不同的AC/DC及两字节缓存。自动检查在已授权范围复用启动/唤醒/接电通知，不声明能力就不执行。
- `hal/beep_audio`复刻实际DLL分组：名称来自Volume节点，Mute来自同组节点；第一PC Beep组、原枚举顺序、分叉/SUM和终端边界均保留。使用同一IAudioMute读取、写入及回读；首次实际修改前原子保存设备身份和前态，不加载原DLL或修改主扬声器静音。
- 静音循环为自生成双声道16bit、44100Hz、22528帧零样本，与原nosound资源格式一致。一秒单调计时从播放器启动成功后开始，重复186不延期；187停循环并置静音，但保留已排队一次通知。客户非5或EC两个低电量位都为0时发121/44/data1，只尝试一次。
- 停止策略、退出、许可撤销或阶段失败取消待发任务，独立尝试停止本进程循环和恢复首次codec。来源/许可在阶段间及循环运行中持续复核；清理失败保留文件、错误和确认重试入口。热键工作线程明确区分执行错误与清理结果，stop等待结束并返回清理错误。
- UI采用全局Lucide图标和左右Surface事实卡；音频为离散状态，亮度为真实百分比。186尚未收到的绝对目标不画成0%；音频初始恢复走类型化IPC及二次确认，亮度恢复共用已有入口。

## 原厂证据

- 安装包：`C:/Program Files (x86)/ControlCenter/AppInstall/FnKey/FnKey.appxbundle`中的`FnKeyPack_7.19.5.0_x64.appx`，包名CLEVOCO.FnhotkeysandOSD，版本7.19.5.0。
- FnKey/FnKey.exe SHA256：`82930cbe1c1caefbf71c84f839248559ad5821bcfaa3faf1e7b4dd6e0f769578`，与Pi会话展开程序集一致；FnKey/audio10ec.dll SHA256：`a5e41376e9763831ccd232837b54381fc5b570144c893ea95df7b7de849ce02a`。
- Pi会话`9b0d7323-5294-4777-8be6-bbeabdcfb6a8`的Form1、Features、DCHU及FnKey.Device.Brightness。PCbeep是WMI70 bit13，不能误用82。Read_ECRAM(1361,1)实际请求为cmd4包`[1,5,0x51,1,0,0,177]`，响应byte1；不是原始端口扫描。
- DLL唯一导出pc_beep_Mute在RVA0x2540。0x3560接IDeviceTopology/connector0/ConnectedTo/IPart；0x3720沿Incoming分组，Software_IO/Fixed终止，分叉或KSNODETYPE_SUM建立组；0x3afd捕获IAudioMute，0x4690写Mute，0x4660读取Mute。组名来自IAudioVolumeLevel，不来自Mute节点名称。
- [WinForms Timer源码](https://github.com/dotnet/dotnet/blob/b0f34d51fccc69fd334253924abd8d6853fad7aa/src/winforms/src/System.Windows.Forms/System/Windows/Forms/Timer.cs)的Start只设置Enabled，相同值直接返回。结合原LoopBeeperStop不停止Send_D3E_State，可确定187后的待发通知不能被错误取消或延期。
- [IAudioMute::GetMute](https://learn.microsoft.com/en-us/windows/win32/api/devicetopology/nf-devicetopology-iaudiomute-getmute)及[IPart::EnumPartsIncoming](https://learn.microsoft.com/en-us/windows/win32/api/devicetopology/nf-devicetopology-ipart-enumpartsincoming)提供读取与合法叶节点边界。

## 影响与验证

173项热键领域回归、17项核心事件检查通过；时序修正后28项音频/codec/原厂包聚焦检查再次通过。覆盖原始顺序、独立范围、已有缓存、绝对值、共享首次基线、客户5、187保留通知、取消、首次及部分失败、设备替换、停止失败重试、持续控制权检查、一次通知不重复写入，以及延迟从播放成功后开始。

12项电池音频/亮度模拟页面通过；增加无调暗能力的绝对事件、停止失败重试及音量图标后11项音频/全局图标页面通过。前端构建、桌面workspace离线检查和control-sim全部目标编译通过。6项拓扑/PCM格式检查沿用此前通过结果，其源代码未改变；深浅宽窄截图按当前事实卡检查。

证据：`target/ui-check/battery-alert-evidence.json`记录当前源码摘要、检查分组和实际日志；最终热键/工作区为`battery-alert-stop-final.log`及`battery-alert-stop-workspace-final.log`，前端和模拟宿主为`battery-alert-current-build.log`及`battery-alert-current-sim.log`，页面为`battery-alert-stop-ui.log`及`battery-alert-render.log`。核心事件与时序另见`battery-alert-core-final.log`和`battery-alert-clock-final.log`；安装包哈希清单为`installed-fnkey-package.json`。重叠回归不累计为新增用例数。

只运行内存状态、注入函数桩、私有临时记录/线程和模拟IPC。没有枚举实际音频端点、播放声音、读取EC、发送WMI、读取真实OutLog或运行原厂DLL。现存NSIS仍为2026-11-15的旧阶段包，需在原12项最终核对后重建；整个目标仍未以本模块完成替代。

## 风险与备选

原DLL导出忽略部分内部结果，本实现保留错误与回读；控件不可读时拒绝输出。硬件与原生Core Audio事件效果未经物理验收，不能将隔离检查表述为真机通过。不采用主音量替代、原厂初始化值冒充实际前态、未知能力默认开启或为未实现控制提供假开关。
