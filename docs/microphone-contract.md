# 麦克风静音契约

## 来源与范围

Pi真实反编译的FnKey.Form1.Arg001_GetEvent(38)创建DgMic，后者枚举ECapture、DeviceStateActive并逐端点调用AudioEndpointVolume.SetMute；GetMute取得实际系统状态。Features.SetCallMuteStatusToRegistry只保存page1[41]。CallingVariations中的音量函数声明没有进入此链，不能视为播放音量功能。

Core Audio边界按Microsoft的[端点枚举](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nf-mmdeviceapi-immdeviceenumerator-enumaudioendpoints)、[端点音量接口](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudioendpointvolume)及[设备名称](https://learn.microsoft.com/en-us/windows/win32/coreaudio/device-properties)契约实现，使用已有锁定版本windows 0.62.2的生成接口。没有采集、播放或录音代码。

## 所有权与数据

- `hal/audio.rs`只拥有活动录音端点的ID、可用名称和静音读写。COM初始化/释放配对，返回字符串和PROPVARIANT明确释放；名称读取失败保留日志并返回未知名称，静音读取失败不补默认值。
- `microphone/`唯一拥有Snapshot、Target、Scope/Plan、首次基线和补偿。ID完整排序、唯一且无空值/NUL；最多128个端点。完整集合绑定预览及热键授权，不自动接管插入的新设备。
- Snapshot分别报告逐端点系统状态及原始OEM保存字节。Target保存逐端点目标和布尔偏好，允许档案保留混合静音状态；名称只是预览元数据，执行保留当前设备名称。捕获遇到未知OEM偏好拒绝生成可执行目标，恢复可精确还原未知原始字节。
- 读取不执行原厂初始化时的隐式RestoreCallMuteStatus，不根据保存偏好伪造系统状态，也不把外部应用的状态变化自动写回原厂保存块。

## 预览、执行与恢复

单项请求为`{expectedRevision, muted}`，明确全部静音或全部取消静音；缺省保留当前，不自动生成目标。修订绑定当前完整快照与首次基线；读取失败、外部变化和新设备使旧确认失效。各入口持有相同领域Scope后再取得HAL控制锁，原厂控制者仍运行时不写入。

写入前记录每个尝试端点及原保存字节，逐端点写后回读，最后保存OEM偏好并核对。失败分别尝试恢复记录中的端点与保存字段，恢复成功即移除对应尝试；未尝试的端点不回放。设备断开和恢复失败保留具体错误，可重试；保存字节已被外部改成另一值时拒绝覆盖。恢复只要求初始设备仍然存在，合并当前名称与初始静音状态，额外新设备保持当前状态；初始设备断开或替换时保留记录并拒绝旧恢复。成功后清除本会话基线，不恢复其他域字段。

完整档案的`microphones`使用同一Target和Scope，捕获、严格导入导出、手动和启动执行一致。全量预检先检查集合；显示试用保留后才执行。后续域失败按本次尝试记录恢复，同时保留单项/热键首次基线供用户恢复。

## 热键与界面

38号事件为独立`microphoneMute`授权范围，不包含在键盘灯或唤起授权中。完全静音时取消静音；混合状态明确统一静音。执行基于当时系统状态，设备集合变化后须重新确认接管范围；操作结果复用全局OSD和控制台事实刷新。

控制台左侧列出实际逐端点状态，右侧编辑目标；只显示本次变化。卡片保留模块边框和间距，窄屏纵向排列。档案与启动摘要逐端点展示，混合状态不压成统一开关。首次恢复只覆盖当前应用会话，重开不自动执行保存偏好。

## 验证边界

使用明确内存音频端点、共享模拟OEM保存页、临时档案文件和模拟IPC；单元测试未配置传输时明确拒绝原生回退。Windows接口仅静态审查和编译，没有枚举实际设备、修改系统静音、调用DCHU、读取真实热键或录音。实际音频驱动、设备插拔及原厂后台共存效果仍单独验收。
