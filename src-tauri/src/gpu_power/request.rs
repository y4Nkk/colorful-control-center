//! 授权只携带冻结的设备事实；就绪不等于允许访问设备。
use super::sequence::Sequence;
use super::{Device, Plan, Target, operation};
use crate::hal::authorization::session::{self, Authorized, Binding, Session, Task};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::PathBuf};

pub(super) const LIMIT: usize = 16 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    pub expected: Device,
    pub target: Target,
    pub restore: bool,
}
impl Request {
    pub fn plan(&self) -> Plan {
        Plan {
            id: self.expected.id.clone(),
            expected: self.expected.target(),
            target: self.target,
        }
    }
    pub fn encoded(&self) -> Result<Vec<u8>> {
        self.plan().validate()?;
        if self.expected.name.len() > 1024 || self.expected.name.contains('\0') {
            bail!("原厂独显设备名称无效");
        }
        operation::preflight(&self.expected, self.expected.target(), self.target)?;
        if self.restore && !self.target.enabled && !self.expected.can_disable {
            bail!("恢复关断会影响当前显示输出");
        }
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > LIMIT {
            bail!("原厂独显授权请求过大");
        }
        Ok(bytes)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Invocation {
    pub session_dir: PathBuf,
    pub request_hash: String,
    pub authorization: Binding,
}
impl Invocation {
    fn validate(&self) -> Result<()> {
        self.authorization.validate()?;
        if self.request_hash.len() != 64
            || !self.request_hash.bytes().all(|c| c.is_ascii_hexdigit())
        {
            bail!("原厂独显请求摘要无效");
        }
        let directory = self.session_dir.canonicalize()?;
        let prefix = format!(".gpu-session-{}-", self.authorization.token);
        if directory.parent() != Some(std::env::temp_dir().canonicalize()?.as_path())
            || !directory
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.starts_with(&prefix))
        {
            bail!("原厂独显会话目录不属于本次请求");
        }
        Ok(())
    }
    pub fn arguments(&self) -> Result<String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > 14_000 {
            bail!("原厂独显授权参数过大");
        }
        Ok(format!(
            "--oem-gpu-power {}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ))
    }
    pub fn from_args(args: &[String]) -> Result<Self> {
        if args.len() != 2
            || args[0] != "--oem-gpu-power"
            || args[1].len() > 28_000
            || args[1].len() % 2 != 0
            || !args[1].bytes().all(|c| c.is_ascii_hexdigit())
        {
            bail!("原厂独显授权请求编码无效");
        }
        let bytes = args[1]
            .as_bytes()
            .chunks_exact(2)
            .map(|v| u8::from_str_radix(std::str::from_utf8(v)?, 16).map_err(anyhow::Error::from))
            .collect::<Result<Vec<_>>>()?;
        let invocation: Self = serde_json::from_slice(&bytes)?;
        invocation.validate()?;
        Ok(invocation)
    }
    pub fn request(&self) -> Result<Sequence> {
        self.validate()?;
        let mut bytes = Vec::new();
        std::fs::File::open(self.session_dir.join("request.json"))?
            .take((LIMIT + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > LIMIT || hash_bytes(&bytes) != self.request_hash {
            bail!("原厂独显请求已变化或过大");
        }
        let request: Sequence = serde_json::from_slice(&bytes)?;
        request.encoded()?;
        Ok(request)
    }
}
fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
fn prepare_request(request: Sequence) -> Result<(Binding, tempfile::TempDir, String)> {
    let bytes = request.encoded()?;
    let binding = Binding::new()?;
    let directory = tempfile::Builder::new()
        .prefix(&format!(".gpu-session-{}-", binding.token))
        .tempdir()?;
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
pub(crate) fn prepare(request: Sequence) -> Result<Box<dyn Authorized>> {
    prepare_with(request, crate::hal::authorization::launch)
}
pub(crate) fn prepare_with(
    request: Sequence,
    launch: impl FnOnce(&str) -> Result<u32> + Send + 'static,
) -> Result<Box<dyn Authorized>> {
    let steps = request.requests.len();
    let (binding, directory, arguments) = prepare_request(request)?;
    Session::prepare(binding, directory, arguments, steps, launch)
}
pub(crate) fn serve(
    invocation: &Invocation,
    prepare: impl FnMut(Request) -> Result<Box<dyn Task>> + Send + 'static,
) -> Result<()> {
    invocation.request()?;
    session::serve(&invocation.authorization, &invocation.session_dir, || {
        Ok(Box::new(
            super::sequence::Execution::new(invocation.clone(), prepare)
                .context("独显执行许可后的请求复核失败")?,
        ))
    })
}
