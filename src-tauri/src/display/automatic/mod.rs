//! 自动刷新率的原厂设置字段与持久恢复。
#[cfg(test)]
#[path = "scheduler/completed_tests.rs"]
mod completed_tests;
mod controller;
mod engine;
#[cfg(test)]
mod engine_tests;
mod facts;
#[cfg(test)]
mod facts_tests;
mod fields;
#[cfg(test)]
mod fields_tests;
mod native;
#[cfg(all(not(test), not(control_sim)))]
mod runtime;
mod scheduler;
#[cfg(test)]
mod scheduler_tests;
mod state;
pub(crate) use controller::{Controller, Status};
#[cfg(test)]
mod state_tests;
