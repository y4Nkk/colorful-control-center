//! 面向 UI 的命令层（薄委托）：只做参数校验与调度，领域逻辑由顶层领域
//! 模块拥有；commands/ 不得沉淀业务状态。
//!
//! 子模块契约：
//! - `app`：应用自身信息（版本、名称）
//! - `hardware`：硬件监控快照（薄委托 → hal）
//! - `fans`：风扇曲线/模式/偏移（薄委托 → fanctl）
//! - 后续硬件领域（power / gpu…）按域开子模块，领域逻辑放
//!   `src-tauri/src/<domain>/`，命令层保持可替换的薄壳。

pub(crate) mod app;
pub(crate) mod fans;
pub(crate) mod hardware;
