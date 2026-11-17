use super::super::store::Store;
use super::*;
use std::{
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

impl runtime::Input for event_log::Log {
    fn poll(&mut self) -> Result<Vec<event_log::Record>> {
        self.poll()
    }
    fn discard_pending(&mut self) -> Result<()> {
        self.discard_pending()
    }
}
fn start_program(program: &str) -> Result<()> {
    crate::hal::oem_owner::ensure_owner_absent()?;
    Command::new(program)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|error| anyhow!("程序宏启动失败：{error}"))?;
    Ok(())
}
pub(super) fn prepare(
    path: PathBuf,
    record: Record,
    programs: Vec<Program>,
    recording: Arc<recording::Controller>,
    gate: Arc<Mutex<()>>,
) -> Result<Session> {
    if record.phase != Phase::Applied {
        bail!("程序监听目标必须是最终提交配置");
    }
    let control = Arc::new(runtime::Control::default());
    let shared = control.clone();
    let expected = record.clone();
    let (ready, receiver) = mpsc::channel();
    let handle = thread::Builder::new()
        .name("ccc-program-macros".into())
        .spawn(move || {
            let result = (|| {
                let mut log = event_log::Log::open("System")?;
                let _ = ready.send(Ok(()));
                runtime::run(&mut log, &shared, &gate, |event| {
                    dispatch(
                        event,
                        &programs,
                        &expected,
                        || recording.active(),
                        || Ok(super::super::store::FileStore(path.clone()).load()?.record),
                        start_program,
                    )?;
                    Ok(())
                })
            })();
            if let Err(error) = result {
                let message = format!("{error:#}");
                shared.fail(error);
                let _ = ready.send(Err(message));
            } else {
                shared.pause();
            }
        })?;
    let session = Session {
        control,
        thread: Some(handle),
        record,
    };
    match receiver.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(())) => Ok(session),
        Ok(Err(error)) => Err(anyhow!(error)),
        Err(error) => Err(anyhow!("程序事件监听未就绪：{error}")),
    }
}
