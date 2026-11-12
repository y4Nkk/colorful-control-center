//! ccc-core：纯逻辑核心 crate。
//!
//! 契约（对齐 agent-switchboard 三层 crate 模式）：
//! - 本 crate 只含可测试的纯函数与纯状态机（风扇曲线引擎、后续策略计算）
//! - 禁止任何文件 / 网络 / 硬件副作用；硬件写入唯一经由 src-tauri 的 HAL
//! - 单元测试放同文件 `#[cfg(test)]`，`cargo test -p ccc-core` 即可全跑

pub mod fan;
