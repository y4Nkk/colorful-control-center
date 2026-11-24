# 触摸板开关契约

## 原厂依据

Pi会话9b0d7323-5294-4777-8be6-bbeabdcfb6a8的真实FnKey.Features.GetI2C_TP/SetI2C_TP及CC40.Interface_Preference从HKCU的`SOFTWARE\Microsoft\Windows\CurrentVersion\PrecisionTouchPad\Status`读取DWORD `Enabled`，以Ctrl+Win+F24切换系统开关，然后保存page1[10]。保存字节不是系统执行命令。

FnKey.Device.DeviceControl.FoundI2CHID检查HID类、描述包含I2C且节点可查询。当前实现枚举相同HID类的在场节点，并要求Windows状态明确；完整识别节点集合绑定预览，不把单个通用I2C节点冒充精确触摸板型号。

Microsoft的[触摸板切换契约](https://learn.microsoft.com/en-us/windows-hardware/design/component-guidelines/touchpad-enable-or-disable-toggle-button)明确Ctrl+Win+F24语义，系统状态键只读；[旧设备设置接入](https://learn.microsoft.com/en-us/windows-hardware/design/component-guidelines/touchpad-legacy-touchpad-pc-settings-opt-in)解释逐用户HKCU状态。输入遵守[SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)串行发送、按键状态及UIPI限制。应用不修改系统状态注册表、不禁用PnP设备、不调用TPStatus。

## 所有权与目标

- `hal/touchpad.rs`拥有原生设备识别、只读系统状态、组合键发送及最长2秒目标回读。读到缺失、类型不匹配或非0/1的状态不补默认值。发送前核对节点及实际开关；已有修饰键/F24按下时拒绝。发送不完整时释放本次成功按下的键，报告具体错误，不假报目标生效。
- `touchpad/`拥有Snapshot、Target、Scope/Plan和补偿；Scope以写互斥锁串行化单项、热键与档案事务。测试必须显式注入IO，原生模块不编入测试路径。
- Snapshot保留排序且唯一的完整节点ID集合、可空系统开关和原始OEM保存字节。最多128个节点，身份非空且无NUL。Target独立保存`devices/enabled/savedEnabled`，允许档案保持系统值与保存偏好不同；未知保存字节拒绝生成可执行捕获。
- 读取不执行原厂启动恢复，不初始化保存值，也不自动把外部系统变化写回OEM设置。

## 执行与恢复

单项请求为`{expectedRevision, enabled}`，只在明确选择并确认后执行；修订绑定当前完整快照。单项、热键与档案均先取得同一领域Scope，再取得HAL控制锁，档案事务持有Scope期间其他写入者等待，原厂控制者仍在时禁止修改。

全量预检核对节点集合和实际系统状态。切换前记录尝试状态；实际目标回读成功后才写本域保存字节并核对。无变化不发送输入。任何阶段失败停止，分别恢复已尝试的系统状态及OEM偏好，成功字段移出补偿集合；补偿失败保留尝试记录，外层档案可继续重试，单项操作可重新读取后再次提交。

补偿前核对完整原节点集合，不接管替换或新增节点；补偿时按实际状态决定是否需要切换，避免盲目重复组合键。保存字节若已被外部改成原值和目标之外的值，自动补偿拒绝覆盖。page1其他字段始终保留。

## 热键、界面与档案

FnKey的252/253在已确认I2C条件下均切换；非I2C分支只有屏显。两事件映射到独立`touchpad`授权范围，设备变化撤销授权资格，实际执行复用同一事务与OSD。93调用TPStatus的旧路径和215的OLED/触摸板复合分支没有被当成无条件切换；它们不进入当前写入路由。

设备配置卡片显示系统状态，详情分开展示系统开关和保存偏好。启用或关闭直接提交当前修订并作用于当时的节点集合，写后回读。软件不保存首次基线，也不提供初始恢复入口。读取失败撤销可执行快照，成功回读后恢复切换资格。

完整档案的`touchpad`贯穿捕获、严格文件、导入导出、图形预览、手动及启动应用。显示试用明确保留后才执行其他域；触摸板在麦克风之后、键盘灯之前执行，补偿按相反顺序，后续GPU等失败恢复本次前态。

## 验证边界

内存传输、共享模拟OEM保存页、私有临时档案和模拟IPC覆盖实际差异、旧预览、状态未知、设备变化后继续操作、逐阶段失败、成功但无效果、失败补偿后重试、显示试用和启动执行。Windows接口只做静态审查与编译；未枚举真实HID、读取真实注册表、发送系统输入、调用DCHU或验收物理触摸板。TP颜色属于独立灯光协议，不在此开关域内。
