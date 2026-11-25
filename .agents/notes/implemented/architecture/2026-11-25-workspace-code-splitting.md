# Agent Note: 工作区按首次访问加载

Status: implemented

## 问题

工作区已按首次访问挂载并保留草稿，但所有页面仍静态进入前端首包。本次 Vite 生产基线的入口 JavaScript 为 814,275 字节，概览启动同时加载了性能、设备、键盘和电源页面的代码。

## 决定

在 AppWorkspace 对性能与散热、设备配置、键盘与灯光、电源与档案使用 React `lazy` 和 Vite 动态导入。沿用各页的挂载条件，在原工作区内提供加载状态。概览和等待偏好期间使用的设置页继续由入口加载。

参考 [React lazy](https://react.dev/reference/react/lazy)、[Vite 动态分包](https://vite.dev/guide/features.html#dynamic-import) 与 [VS Code 按需实例化](https://github.com/microsoft/vscode/wiki/source-code-organization#providing-a-service)。

## 影响与验证

- 对比同一工作树前后的 Vite 生产 manifest，入口及其静态依赖的 JavaScript 从 814,275 降至 470,223 字节，减少 42.3%。用同一 gzip 方式逐文件计算，从 238,983 降至 150,152 字节，减少 37.2%。
- 生产构建的 Edge 模拟 IPC 检查通过 27 项。概览只请求静态入口依赖，首次访问键盘页请求对应分包，延迟时显示加载状态。已有回归覆盖上次访问启动页、托盘直达、热键导航、草稿与滚动位置保留。
- `npm run build` 与 `cargo check --offline --workspace` 通过。

## 风险

首次打开拆分后的页面需要加载对应本地分包。真实 WebView2 冷启动和首次切页耗时待真机测量。

## 备选方案

先拆分所有静态样式将扩大样式所有权变更，留待首屏剖析。全量立即预取会增加概览启动时的资源处理，按首次访问加载即可和原挂载边界对齐。
