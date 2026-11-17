# Agent Note: 控制进程与监护的跨进程控制权

Status: implemented

## 问题

HAL 的控制锁只在单进程内生效。此前主程序允许重复启动，多个软件曲线与档案事务可交错；主进程被强杀后，旧监护恢复期间也没有阻止新主进程开始写入。原有进程测试覆盖失联恢复，没有覆盖重复启动和恢复期间重新接管。

## 决定

- 使用 Tauri 官方 single-instance 插件作为首个插件；重复普通启动只唤起现有窗口，重复 --autostart 不再次应用档案或唤起窗口。监护参数分支先于 Tauri，不能被主窗口单实例拦截。
- 增加 Windows 全局命名 mutex 的句柄租约，只以对象存在代表控制权，不使用线程所有权。主进程在初始化硬件之前独占创建；监护在读取硬件及 READY 之前打开并保留同一对象。主进程消失后，监护持有句柄直到恢复结束，禁止新主进程抢先初始化和写入。句柄通过 RAII 自动关闭，不留锁文件或真实用户配置。
- 控制租约与 HAL 初始化放在单实例插件之后、应用窗口创建之前的插件 setup；普通打开仍不执行目标写入。租约失败明确报错，不绕过或杀旧进程。
- 新进程测试只启动专用 fixture，使用每次唯一的隔离 mutex 名称，恢复只写日志；复用生产租约、监护客户端及协议，不启动真实应用、不加载 OEM DLL。
- 2026-11-17安装版暴露监护握手超时：start把--direct-fan-watchdog放在--fan-watchdog之前，而正式入口只按首参数分派，导致启动成普通应用。角色参数必须先于该角色的选项；spawn显式接收direct_required，统一生成角色后选项，全部直接消费者同步，不接受旧参数顺序。隔离进程新增直接恢复分支的严格参数顺序及握手/解除检查，避免fixture的any匹配掩盖正式入口错误。
- 同轮补齐Windows GUI子系统的管道验收：隔离监护fixture使用与正式Release相同的GUI子系统，不加载HAL。工作进程失败通过stdout协议返回父进程，不能仅写GUI不可见的stderr；达到95°C的启动拒绝明确说明保留自动散热，不表述为握手超时。真实硬件仍不执行。

## 影响与验证

- 7 项真实进程隔离检查通过：并发主进程唯一、持有期间拒绝第二个、正常退出和强杀后接管；父进程强杀后监护恢复前拒绝接管，恢复后可接管；监护先死不提前释放存活父进程控制权；显式解除和孤立监护拒绝。
- 既有 watchdog_process 8 项、software_process 5 项真实隔离进程及原生 watchdog:: 10 项回归通过。无硬件、真实应用或用户配置操作。
- 新增官方 tauri-plugin-single-instance 2.5.5，仅增加一个锁定包；核对实际依赖源的插件/窗口初始化顺序，控制租约先于 HAL、窗口及启动档案。重复自启回调不含任何档案应用入口，窗口实际行为不以源码检查代验。
- npm run build、cargo check --workspace、17 篇笔记校验及 diff --check 通过；源文件 ≤500 行、函数 ≤80 行。
- NSIS 构建与 7z t 完整性通过，主程序确认为 x64 Windows GUI。安装包 1894779B，SHA256 2fd570fce31535c5575390a5f89bb5b49c28cc74bb294daf5b8d0b9976e9846e；未安装、启动或操作真实硬件。实际窗口唤起、自启与物理恢复仍待验收。

## 风险

该互斥只协调本软件，不阻止 OEM 后台覆写硬件。跨用户无法取得现存对象权限时拒绝接管；不放宽系统对象 ACL。进程测试不证明驱动或物理风扇恢复。

## 备选方案

- 仅使用 Tauri 单实例：主进程结束会释放锁，无法覆盖仍在恢复的监护，不采用。
- 锁文件/PID：需要处理崩溃陈旧状态及 PID 重用，不采用。
- 让主进程先独占线程 mutex 再让监护等待：不能覆盖父子交接且引入线程所有权，使用共享对象句柄生命周期。

## 依据

- https://v2.tauri.app/plugin/single-instance/
- https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/single-instance
- https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createmutexw
- https://learn.microsoft.com/en-us/windows/win32/termserv/kernel-object-namespaces
