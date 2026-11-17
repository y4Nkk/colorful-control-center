# 原厂屏幕亮度策略契约

## 1. 来源与当前读取通道

事实源为本机FnKeyPack 7.19.5.0及指定Pi会话的FnKey.Form1.Arg001_GetEvent、Features.SetBrightnessFromWMI17/CheckIfCurrentBrightnessIsLower/SetBrightnessSlowly、LCDBrightness和FnKey.Device.Brightness。

78/79读取cmd17包[210]/[211]；48–56依次读取[212..220]。ccc-core/display_brightness唯一拥有11项映射及0–100百分比校验，0是有效目标，不静默改为其他档位或增减步长；事件高位不改变目标选择。完整表绑定接管许可，其他目标改变也使旧许可失效。

原版通过旧LCD IOCTL读取AC/DC报告。Microsoft说明[该接口从Vista起废弃](https://learn.microsoft.com/en-us/windows/win32/power/ioctl-video-query-display-brightness)，当前唯一读取通道为ROOT\WMI下的[WmiMonitorBrightness](https://learn.microsoft.com/en-us/windows/win32/wmicoreprov/wmimonitorbrightness)。必须恰好一个Active实例，绑定InstanceName，接受明确的0–100 CurrentBrightness；多个实例、缺失报告、非法类型或未知供电状态均拒绝执行。没有旧IOCTL、计划值或默认0/100回退。保存值与Windows面板报告分开展示，不据此宣称已验收物理亮度。

hal/wmi统一拥有COM公寓、WMI连接和代理设置，键盘清单与亮度读取共用；service先于公寓释放。亮度原生模块不编入测试，测试只允许显式报告提供者及内存FFI。所有WMI查询都在领域锁和HAL锁外完成，读取热键配置也不持软件修改锁等待查询；等待和取消不占用风扇控制锁。

## 2. 独立许可与计划副作用

screenBrightness是第十五项独立接管范围，默认关闭。键盘亮度、背光循环、状态通知及摄像头许可都不包含屏幕策略。确认前展示面板报告、供电、活动计划AC/DC现值和完整11项触发目标；包含0%的目标明确提示可见性影响。

授权绑定面板实例、活动计划和完整目标表，不绑定瞬态亮度、供电或本策略修改后的保存值。读取配置不写入计划或建立恢复记录。原厂控制权及软件文件许可沿用hotkeys统一校验，执行前及输出前后重新核对。

原版Write_Brightness依次写活动计划的AC和DC字段。当前同一事务写这两个字段并回读，最后刷新仍为活动状态的该计划，失败补偿已尝试部分；不切回已失活的计划。hal/power_plan的PowerAccess唯一执行系统电源读写，使用VIDEO子组及[VIDEONORMALLEVEL](https://learn.microsoft.com/en-us/windows-hardware/customize/power-settings/display-settings-display-brightness-level)，不改自适应亮度、ASPM、USB或其他计划字段。

## 3. 可取消的执行与失败恢复

插电或面板当前报告已经低于/等于目标时零输出，不建立初值。电池状态需降低时，每秒等待后重新核对来源；最多三轮写入，最后再读取一次报告。达到目标或供电转插电结束后续输出；未达到目标则停止重复写入并恢复本次前态。系统查询本身返回后再次检查许可，不在取消之后继续写入。

事件线程串行处理该有界操作，停止信号可中断等待。许可撤销、停止、面板/计划/目标表变化或任一步失败，恢复本次已尝试字段。每个字段在调用之前登记；在首字段之前取消时零写入、零计划刷新、零初始记录，也不为无输出的取消再启动面板查询。停止信号只由此上下文消费并传回外层结束监听，补偿失败不能因取消而丢失诊断。

每个实际接管计划只有一份本会话初值及字段掩码；不同计划独立记录，后续成功或失败不覆盖最初值。补偿保留其他字段，原面板替换时保留记录并拒绝向新面板恢复。通过预检后，各字段及计划独立尝试，错误同时保留；失败后可重新预览重试。

## 4. 初始恢复与界面

hotkeys/screen_brightness唯一拥有初值、来源绑定、执行及恢复。read_hotkeys的facts.screenBrightness提供可空当前事实、明确错误、逐计划初始/当前字段、接管掩码及独立restoreRevision。目标包暂时不可用时，仍可用已知面板和计划信息预览并恢复初值；未知当前计划字段不伪造为零。

低电量事件59/60另有独立授权和持久恢复记录，复用同一控制器与锁；记录存在时本策略的执行和初始恢复暂停，先恢复低电量接管字段。规则、记录和供电条件由[电池契约](battery-contract.md#原厂低电量亮度事件)拥有。

restore_screen_brightness先校验恢复预览，再停止本次监听，随后再次核对并恢复已接管字段。停止过程中发生补偿会改变来源时要求重新预览，不使用旧许可覆盖新值；失效的最初预览不停止已有监听。恢复当前活动计划时刷新，恢复非活动计划只还原其字段。保存的软件接管规则保留，软件规则恢复和硬件初始恢复分别确认。

ScreenBrightnessFacts复用MetricReadout的百分比环、FactList和ConfirmButton；宽卡内实际读数与计划事实左右排列，窄卡折行，保留模块边框和间隔。失败不显示假读数，0%目标在确认前可见，恢复预览逐字段显示变化及未接管字段。

## 5. 验证边界

使用显式内存报告、DCHU ABI函数桩、私有状态与模拟IPC；没有查询真实面板/WMI/供电或修改Windows计划。原生提供者仅编译核对，物理亮度、OEM事件及实际供电变化尚未验收。聚焦检查及当前安装包证据记录在acceptance-checklist.md。
