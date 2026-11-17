use super::{
    Native, hash,
    model::{self, Record, State},
    transaction::{self, Io},
};
use crate::hal::authorization::{
    self,
    session::{self, Binding, Session, Task},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Operation {
    pub expected: State,
    pub record: Option<Record>,
    pub target: State,
    pub executable_hash: String,
    pub user: String,
    pub record_path: PathBuf,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Invocation {
    binding: Binding,
    directory: PathBuf,
    digest: String,
}
pub(super) fn prepare(operation: Operation) -> Result<Box<dyn session::Authorized>> {
    let binding = Binding::new()?;
    let parent = operation.record_path.parent().context("登录记录没有目录")?;
    std::fs::create_dir_all(parent)?;
    let directory = tempfile::Builder::new()
        .prefix(&format!(".login-{}-", binding.token))
        .tempdir_in(parent)?;
    let bytes = serde_json::to_vec(&operation)?;
    if bytes.len() > 262144 {
        bail!("登录任务请求过大");
    }
    crate::storage::write(&directory.path().join("request.json"), &bytes)?;
    let invocation = Invocation {
        binding: binding.clone(),
        directory: directory.path().into(),
        digest: format!("{:x}", Sha256::digest(&bytes)),
    };
    let bytes = serde_json::to_vec(&invocation)?;
    let args = format!(
        "--configure-login {}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    Session::prepare(binding, directory, args, 1, authorization::launch)
}
impl Invocation {
    fn operation(&self) -> Result<Operation> {
        self.binding.validate()?;
        let mut bytes = Vec::new();
        std::fs::File::open(self.directory.join("request.json"))?
            .take(262145)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 262144 || format!("{:x}", Sha256::digest(&bytes)) != self.digest {
            bail!("登录任务请求已变化");
        }
        let operation: Operation = serde_json::from_slice(&bytes)?;
        let root = super::record_path()?
            .parent()
            .context("缺少用户目录")?
            .canonicalize()?;
        let directory = self.directory.canonicalize()?;
        if operation.record_path.file_name().and_then(|v| v.to_str()) != Some(super::RECORD)
            || operation
                .record_path
                .parent()
                .context("缺少恢复目录")?
                .canonicalize()?
                != root
            || directory.parent() != Some(root.as_path())
            || !directory
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.starts_with(&format!(".login-{}-", self.binding.token)))
            || operation.user != super::native::user()?
            || operation.executable_hash != hash()?
        {
            bail!("登录任务请求不属于当前程序、用户或会话");
        }
        let native = Native::new(operation.record_path.clone())?;
        model::validate(&operation.target, &native.executable, &native.user)?;
        if native.state()? != operation.expected || native.record()? != operation.record {
            bail!("登录任务前态已变化");
        }
        Ok(operation)
    }
}
struct Execution {
    operation: Operation,
}
impl Task for Execution {
    fn execute(&mut self) -> Result<()> {
        let o = &self.operation;
        transaction::apply(
            &Native::new(o.record_path.clone())?,
            &o.expected,
            &o.record,
            &o.target,
        )
    }
    fn rollback(&mut self) -> Result<()> {
        Ok(())
    }
    fn dirty(&self) -> bool {
        // 配置是已确认的持久操作，事务自行补偿并落盘失败证据；父进程结束不撤销已保存配置。
        false
    }
}
fn execute(args: &[String]) -> Result<()> {
    if args.len() != 2
        || args[0] != "--configure-login"
        || args[1].len() > 28000
        || args[1].len() % 2 != 0
    {
        bail!("登录授权参数无效");
    }
    let bytes = args[1]
        .as_bytes()
        .chunks_exact(2)
        .map(|b| Ok(u8::from_str_radix(std::str::from_utf8(b)?, 16)?))
        .collect::<Result<Vec<_>>>()?;
    let invocation: Invocation = serde_json::from_slice(&bytes)?;
    let result = (|| {
        authorization::require_elevated()?;
        let operation = invocation.operation()?;
        session::serve(&invocation.binding, &invocation.directory, move || {
            Ok(Box::new(Execution { operation }))
        })
    })();
    if let Err(error) = &result {
        session::report_failure(&invocation.directory, &format!("{error:#}"))?;
    }
    result
}
pub(crate) fn run_child(args: &[String]) -> u32 {
    match execute(args) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("[configure-login] {error:#}");
            1
        }
    }
}
