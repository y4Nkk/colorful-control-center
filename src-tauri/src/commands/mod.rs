//! 面向 UI 的命令层（薄委托）：只做参数校验与调度，领域逻辑由顶层领域
//! 模块拥有；commands/ 不得沉淀业务状态。
//!
//! 子模块契约：
//! - `app`：应用自身信息（版本、名称）
//! - `window`：自定义标题栏窗口操作与最大化状态
//! - `hardware`：硬件监控快照与只读 CPU/内存/磁盘卷概览（薄委托 → hal）
//! - `devices`：刷新率试用、原厂独显电源、MUX模式及选择记录与恢复
//! - `fans`：固件曲线/软件自由曲线/模式/偏移（薄委托 → fanctl）
//! - `dust`：固件除尘启动、真实状态及定时计划与恢复
//! - `power`：功耗墙/电压偏移、原厂模式及Turbo风扇/DTT选项（薄委托 → powerctl）
//! - `supply_mode`：原厂六条件供电模式预览、启用、暂停、继续和初始恢复
//! - `gpu`：NVAPI 偏移与原始基线恢复
//! - `lighting`：键盘静态/动态/多区灯效、按键响应、闲置/开机、外围灯区与恢复
//! - `lightbar`：X170独立灯条HID模式、亮度及恢复
//! - `keyboard`：原厂Fn锁、Win锁和Fn/Win交换设置
//! - `microphone`：活动录音端点静音、原厂偏好、本会话首次恢复
//! - `keymap`：FlexiKey映射配置、过滤驱动事务及持久恢复
//! - `hotkeys` / `osd`：授权原厂热键策略与只读桌面屏显
//! - `platform`：PD档位预览/回读/恢复与Deep S5只读状态
//! - `usb`：固件USB建议及标准Windows计划的选择性暂停事务
//! - `lan`：网卡驱动节能/唤醒补丁、权威预览、授权执行及持久恢复
//! - `battery`：电池读数、充电保护、原厂节能时段与初始恢复
//! - `calibration`：独立电池校准任务的准备、开始、停止与恢复
//! - `profiles`：档案存取、模式/显示/独显/MUX只读预览与确认应用流水线
//! - `settings`：开机自启（HKCU Run）+ 档案导入导出
//! - `preferences`：软件偏好读取、事务保存、字体枚举、快捷键录入和重启
//! - 硬件领域按域开子模块，领域逻辑放
//!   `src-tauri/src/<domain>/`，命令层保持可替换的薄壳。

pub(crate) mod airplane;
pub(crate) mod app;
pub(crate) mod battery;
pub(crate) mod calibration;
pub(crate) mod devices;
pub(crate) mod dust;
pub(crate) mod fans;
pub(crate) mod gpu;
pub(crate) mod hardware;
pub(crate) mod hotkeys;
pub(crate) mod keyboard;
pub(crate) mod keymap;
pub(crate) mod lan;
pub(crate) mod lightbar;
pub(crate) mod lighting;
pub(crate) mod microphone;
pub(crate) mod osd;
pub(crate) mod platform;
pub(crate) mod power;
pub(crate) mod preferences;
pub(crate) mod profile_policy;
pub(crate) mod profiles;
pub(crate) mod settings;
pub(crate) mod supply_mode;
pub(crate) mod touchpad;
pub(crate) mod usb;
pub(crate) mod window;
