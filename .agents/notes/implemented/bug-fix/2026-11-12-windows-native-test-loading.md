# Agent Note: 修复 Windows 原生测试加载并验证监护进程

Status: implemented

## 问题

完整 Tauri 单元测试此前在进入 Rust 测试函数前以 0xc0000139 退出。当前 PE 静态证据：发布 exe 有 Microsoft.Windows.Common-Controls 6.0 清单，测试 exe 没有；二者都直接导入 comctl32!TaskDialogIndirect。tauri-winres 经 embed-resource::compile 只给 bin 链接资源，lib 单元测试遗漏清单。单靠 control-sim 无法证明真实桌面依赖图可加载，也尚未覆盖独立监护的操作系统进程/管道链路。

## 决定

- Windows 清单由 src-tauri/windows.manifest.xml 唯一拥有，保留 Tauri 默认 Common Controls v6 内容。Tauri 继续生成图标/版本资源；关闭其内置清单，使用已在依赖图中的 embed-resource 将单独清单资源链接到所有目标，避免资源重复和复制两份清单。
- 先运行生产 crate 的聚焦内存测试及 --list，验证测试 exe 实际加载，不初始化硬件或 UI。
- 隔离监护测试只替换传感输入和恢复出口，复用生产状态机与管道传输；以本地临时目录记录模拟恢复，不加载 OEM DLL、不运行桌面应用、不写真实配置。
- 发布和测试产物各只有一个相同的 Common Controls v6 清单；旧的“完整测试无法加载”限制已删除。单独图标/版本资源仍由 Tauri 生成。
- 监护客户端将回报管道断开视为失败，结束未 READY 的子进程并保留原握手与清理错误；退出时短暂等待读取线程投递末条诊断。ARM 后的进程仍由断连恢复协议结束，不强杀已承担恢复责任的看门狗。
- 协议输出不因父进程强杀后的 BrokenPipe panic。GPU 依赖状态覆盖 ARM 后才首次出现的 GPU，再次丢失时恢复；不只检查启动时存在的 GPU。
- 工具的 fixture 只使用生产 client/protocol，不引用 HAL；Windows 进程仅查询/结束自己创建的句柄，临时目录删除前校验 canonical parent，失败保留日志。fixture 禁止重复编译为独立单元测试目标。

## 影响与验证

- 生产 crate 的风扇 10 项与监护 9 项聚焦测试已实际启动并通过；测试清单可加载枚举 70 项，未运行完整套件。
- 8 项真实 Windows 进程测试通过，覆盖显式解除、强杀父进程、开管道无心跳、子进程异常退出、子进程仍存活但输出断开、传感器失败/晚出现的 GPU 再丢失、三次恢复失败、初始传感器无效及 READY 超时结束未 ARM 子进程。
- cargo check、npm run build、verify-notes 已通过。清单以单独资源链接所有目标；本地 NSIS 构建与 7z 完整性检查通过，归档含本次编译的 exe 和 WebView2Loader.dll。清单对比与原生枚举结果在本会话 control-verification/windows-manifests.json、native-test-list.json。测试结果不扩张为物理风扇恢复验收。

## 风险

原生测试成功只证明依赖加载和被选测试路径；真实窗口、驱动调用、硬件响应仍未验证。监护进程若加载真实 OEM DLL 仍受驱动阻塞影响，隔离测试不能证明这种情况。

## 备选方案

- 永远只运行 control-sim：保留原生测试缺口，拒绝。
- 假定发布 exe 也缺清单：与当前产物证据矛盾，拒绝。
- 把清单复制到 OUT_DIR 或为测试临时修改系统 SxS 配置：增加隐含状态且不能稳定复现，拒绝。

## 依据

- Cargo 官方 build script 链接目标契约：https://doc.rust-lang.org/cargo/reference/build-scripts.html。
- tauri-build WindowsAttributes 的 Common Controls v6 要求；本机锁定版本 2.6.3 源码及 embed-resource 3.0.11 的 compile/compile_for_everything 实现。
