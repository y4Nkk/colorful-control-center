//! LAN请求绑定属于本域；保留授权的传输和生命周期由HAL统一拥有。
use super::worker::{Invocation, Operation, hash_bytes};
use crate::hal::authorization::session::{self, Session};
use anyhow::{Context, Result};
pub(crate) use session::{Authorized, Binding, Task, report_failure};

fn request(operation: Operation) -> Result<(Binding, tempfile::TempDir, String)> {
    let binding = Binding::new()?;
    let parent = operation
        .recovery_file
        .parent()
        .context("LAN恢复路径缺少父目录")?;
    std::fs::create_dir_all(parent)?;
    let directory = tempfile::Builder::new()
        .prefix(&format!(".lan-session-{}-", binding.token))
        .tempdir_in(parent)?;
    let bytes = operation.encoded()?;
    crate::storage::write(&directory.path().join("request.json"), &bytes)?;
    let arguments = Invocation {
        session_dir: directory.path().into(),
        request_hash: hash_bytes(&bytes),
        authorization: binding.clone(),
    }
    .arguments()?;
    Ok((binding, directory, arguments))
}
#[cfg(not(test))]
pub(crate) fn prepare(operation: Operation) -> Result<Box<dyn Authorized>> {
    prepare_with(operation, crate::hal::authorization::launch)
}
pub(crate) fn prepare_with(
    operation: Operation,
    launch: impl FnOnce(&str) -> Result<u32> + Send + 'static,
) -> Result<Box<dyn Authorized>> {
    let (binding, directory, arguments) = request(operation)?;
    Session::prepare(binding, directory, arguments, 1, launch)
}
pub(crate) fn serve(
    invocation: &Invocation,
    prepare: impl FnOnce() -> Result<Box<dyn Task>>,
) -> Result<()> {
    invocation.operation()?;
    session::serve(&invocation.authorization, &invocation.session_dir, prepare)
}
#[cfg(test)]
#[path = "authorization_tests.rs"]
mod tests;
