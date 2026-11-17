use super::{Target, model::*, native, store, transaction};
use crate::{
    controller_lease::ControllerLease,
    hal::{authorization, oem_dchu::Dchu, oem_owner},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
#[path = "worker/execution.rs"]
mod execution;
pub(crate) use execution::Execution;
pub const MAX_REQUEST_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Intent {
    Apply {
        request: Request,
    },
    Restore {
        expected_revision: String,
        device_id: String,
    },
    Profile {
        expected_revision: String,
        target: Target,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Operation {
    pub intent: Intent,
    pub recovery_file: PathBuf,
}
impl Operation {
    pub fn revision(&self) -> &str {
        match &self.intent {
            Intent::Apply { request } => &request.expected_revision,
            Intent::Restore {
                expected_revision, ..
            }
            | Intent::Profile {
                expected_revision, ..
            } => expected_revision,
        }
    }
    pub fn encoded(&self) -> Result<Vec<u8>> {
        if let Intent::Profile { target, .. } = &self.intent {
            target.validate()?;
        }
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_REQUEST_BYTES {
            bail!("LAN授权请求过大");
        }
        Ok(bytes)
    }
    pub fn plans(&self, frame: &Frame, record: &store::Record) -> Result<Vec<transaction::Plan>> {
        frame.validate()?;
        if revision(frame, record)? != self.revision() {
            bail!("LAN预览、驱动或恢复记录已变化，未写入系统");
        }
        let plans = match &self.intent {
            Intent::Profile { target, .. } => target.plans(frame)?,
            Intent::Restore { device_id, .. } => record
                .restore_plan(frame.device(device_id)?)?
                .into_iter()
                .collect(),
            Intent::Apply { request } => {
                let changes = changes(frame, request)?;
                if changes.is_empty() {
                    Vec::new()
                } else {
                    vec![transaction::Plan {
                        expected: frame.device(&request.device_id)?.clone(),
                        changes,
                    }]
                }
            }
        };
        let mut next = record.clone();
        for plan in &plans {
            plan.validate()?;
            next.remember(plan)?;
        }
        next.encoded()?;
        Ok(plans)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Invocation {
    pub session_dir: PathBuf,
    pub request_hash: String,
    pub authorization: super::authorization::Binding,
}
impl Invocation {
    fn validate(&self) -> Result<()> {
        self.authorization.validate()?;
        if self.request_hash.len() != 64
            || !self.request_hash.bytes().all(|c| c.is_ascii_hexdigit())
        {
            bail!("LAN会话请求摘要无效");
        }
        Ok(())
    }
    pub fn arguments(&self) -> Result<String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > 14_000 {
            bail!("LAN授权参数过大");
        }
        Ok(format!(
            "--oem-lan-policy {}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ))
    }
    pub fn from_args(args: &[String]) -> Result<Self> {
        if args.len() != 2
            || args[0] != "--oem-lan-policy"
            || args[1].len() > 28_000
            || args[1].len() % 2 != 0
            || !args[1].bytes().all(|c| c.is_ascii_hexdigit())
        {
            bail!("LAN授权请求编码无效");
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
    pub fn operation(&self) -> Result<Operation> {
        self.validate()?;
        let path = self.session_dir.join("request.json");
        let mut bytes = Vec::new();
        std::fs::File::open(&path)?
            .take((MAX_REQUEST_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_REQUEST_BYTES || hash_bytes(&bytes) != self.request_hash {
            bail!("LAN会话请求已变化或过大");
        }
        let operation: Operation = serde_json::from_slice(&bytes)?;
        operation.encoded()?;
        let parent = operation
            .recovery_file
            .parent()
            .context("LAN恢复路径缺少父目录")?
            .canonicalize()?;
        let session = self.session_dir.canonicalize()?;
        let prefix = format!(".lan-session-{}-", self.authorization.token);
        if session.parent() != Some(parent.as_path())
            || !session
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with(&prefix))
        {
            bail!("LAN会话目录不属于本次请求");
        }
        Ok(operation)
    }
}
pub fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
fn validate_path(path: &Path) -> Result<()> {
    let config: serde_json::Value = serde_json::from_str(include_str!("../../../tauri.conf.json"))?;
    let expected = PathBuf::from(std::env::var_os("APPDATA").context("未找到用户应用数据目录")?)
        .join(config["identifier"].as_str().context("应用标识缺失")?);
    let parent = path
        .parent()
        .context("LAN恢复路径缺少父目录")?
        .canonicalize()?;
    if path.file_name().and_then(|name| name.to_str()) != Some(super::RECORD_NAME)
        || parent.to_string_lossy().to_lowercase()
            != expected.canonicalize()?.to_string_lossy().to_lowercase()
    {
        bail!("LAN授权仅允许当前用户应用的精确恢复文件");
    }
    Ok(())
}
fn execute(invocation: &Invocation) -> Result<()> {
    let operation = invocation.operation()?;
    validate_path(&operation.recovery_file)?;
    let result = run_authorized(invocation, operation);
    if let Err(error) = &result {
        if let Err(saved) =
            super::authorization::report_failure(&invocation.session_dir, &format!("{error:#}"))
        {
            bail!("{error:#}；保存LAN进程诊断失败：{saved:#}");
        }
    }
    result
}
fn run_authorized(invocation: &Invocation, operation: Operation) -> Result<()> {
    authorization::require_elevated()?;
    let _lease = ControllerLease::retain(crate::controller_lease::NAME)?;
    let _worker =
        ControllerLease::try_acquire(concat!("Global\\", env!("CARGO_PKG_NAME"), ".lan-worker"))?
            .context("已有LAN授权工作进程")?;
    super::authorization::serve(invocation, move || {
        oem_owner::ensure_owner_absent()?;
        let record = store::read(&operation.recovery_file)?;
        let dchu = Dchu::load()?;
        let frame = native::frame(&dchu, !record.devices.is_empty())?;
        Ok(Box::new(Execution::prepare(
            &operation,
            &frame,
            record,
            native::Native,
        )?))
    })
}
pub fn run_child(args: &[String]) -> u32 {
    match Invocation::from_args(args).and_then(|invocation| execute(&invocation)) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("[oem-lan-policy] {error:#}");
            1
        }
    }
}
