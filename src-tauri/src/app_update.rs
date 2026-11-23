use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::windows::fs::OpenOptionsExt,
    path::Path,
    process::Command,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, ipc::Channel};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::{Mutex as AsyncMutex, oneshot};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CandidateInfo {
    id: u32,
    version: String,
    notes: Option<String>,
}

#[derive(Serialize)]
#[serde(
    tag = "phase",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Progress {
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Verifying,
    Launching,
    InstallerOpen,
}

struct Candidate {
    id: u32,
    update: Update,
}

type Cancellation = (u32, oneshot::Sender<()>);

#[derive(Default)]
pub(crate) struct Controller {
    next_id: AtomicU32,
    candidate: AsyncMutex<Option<Candidate>>,
    cancellation: Mutex<Option<Cancellation>>,
}

impl Controller {
    fn cancellation(&self) -> MutexGuard<'_, Option<Cancellation>> {
        self.cancellation
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn begin_download(&self, id: u32) -> (DownloadGuard<'_>, oneshot::Receiver<()>) {
        let (send, receive) = oneshot::channel();
        *self.cancellation() = Some((id, send));
        (DownloadGuard(self), receive)
    }

    fn cancel(&self, id: u32) -> bool {
        let mut slot = self.cancellation();
        if slot.as_ref().is_none_or(|(current, _)| *current != id) {
            return false;
        }
        slot.take()
            .is_some_and(|(_, signal)| signal.send(()).is_ok())
    }
}

struct DownloadGuard<'a>(&'a Controller);

impl DownloadGuard<'_> {
    /// Cancel and installer handoff share this commit point.
    fn commit(&self) -> bool {
        self.0.cancellation().take().is_some()
    }
}

impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        self.0.cancellation().take();
    }
}

pub(crate) async fn check(app: &AppHandle) -> Result<Option<CandidateInfo>> {
    ensure!(!crate::shutdown::closing(), crate::shutdown::CLOSING);
    let key = option_env!("TAURI_UPDATER_PUBLIC_KEY")
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .context("当前构建未配置应用更新公钥，请通过发布页下载正式安装包。")?;
    let controller = app.state::<Controller>();
    let mut candidate = controller
        .candidate
        .try_lock()
        .context("已有更新操作正在进行")?;
    *candidate = None;
    let updater = app
        .updater_builder()
        .pubkey(key)
        .target("windows-x86_64")
        .timeout(Duration::from_secs(300))
        .build()
        .context("初始化更新检查失败")?;
    let Some(update) = tokio::time::timeout(Duration::from_secs(20), updater.check())
        .await
        .context("读取 GitHub 发布清单超时")?
        .context("无法读取 GitHub 发布清单")?
    else {
        return Ok(None);
    };
    ensure!(
        update.download_url.scheme() == "https",
        "安装包下载链接必须使用 HTTPS"
    );
    let id = controller.next_id.fetch_add(1, Ordering::Relaxed);
    let info = CandidateInfo {
        id,
        version: update.version.clone(),
        notes: update.body.clone(),
    };
    *candidate = Some(Candidate { id, update });
    Ok(Some(info))
}

pub(crate) async fn download(
    app: &AppHandle,
    id: u32,
    progress: Channel<Progress>,
) -> Result<bool> {
    ensure!(!crate::shutdown::closing(), crate::shutdown::CLOSING);
    let controller = app.state::<Controller>();
    let candidate = controller
        .candidate
        .try_lock()
        .context("已有更新操作正在进行")?;
    let update = &candidate
        .as_ref()
        .filter(|candidate| candidate.id == id)
        .context("更新信息已变化，请重新检查")?
        .update;

    let (guard, cancelled) = controller.begin_download(id);
    let bytes = tokio::select! {
        biased;
        _ = cancelled => return Ok(false),
        result = fetch_package(update, &progress) => result?,
    };
    if !guard.commit() {
        return Ok(false);
    }

    send(&progress, Progress::Launching);
    let directory = app.path().app_cache_dir()?.join("updates");
    tauri::async_runtime::spawn_blocking(move || launch_installer(directory, bytes, &progress))
        .await?
}

pub(crate) fn cancel(app: &AppHandle, id: u32) -> bool {
    app.state::<Controller>().cancel(id)
}

async fn fetch_package(update: &Update, progress: &Channel<Progress>) -> Result<Vec<u8>> {
    let mut downloaded_bytes = 0u64;
    let mut last_report = Instant::now();
    send(
        progress,
        Progress::Downloading {
            downloaded_bytes,
            total_bytes: None,
        },
    );
    update
        .download(
            |bytes, total_bytes| {
                let first_chunk = downloaded_bytes == 0;
                downloaded_bytes = downloaded_bytes.saturating_add(bytes as u64);
                if first_chunk || last_report.elapsed() >= Duration::from_millis(100) {
                    send(
                        progress,
                        Progress::Downloading {
                            downloaded_bytes,
                            total_bytes: total_bytes.filter(|total| *total > 0),
                        },
                    );
                    last_report = Instant::now();
                }
            },
            || send(progress, Progress::Verifying),
        )
        .await
        .context("下载安装包或校验更新签名失败")
}

fn send(channel: &Channel<Progress>, progress: Progress) {
    let _ = channel.send(progress);
}

fn launch_installer(
    directory: std::path::PathBuf,
    bytes: Vec<u8>,
    progress: &Channel<Progress>,
) -> Result<bool> {
    let operation = crate::shutdown::enter().context(crate::shutdown::CLOSING)?;
    std::fs::create_dir_all(&directory).context("无法创建更新缓存目录")?;
    let mut staged = tempfile::NamedTempFile::new_in(&directory).context("无法暂存安装包")?;
    staged.write_all(&bytes).context("无法写入安装包")?;
    let path = directory.join("ColorfulControlCenter-update.exe");
    let file = staged.persist(&path).context("无法保存已验证的安装包")?;
    drop(file);
    let locked = lock_verified_package(&path, &bytes)?;
    let child = Command::new(&path).spawn().context("无法打开安装向导");
    drop(locked);
    drop(operation);
    let mut child = child?;
    send(progress, Progress::InstallerOpen);

    let status = child.wait().context("无法读取安装向导退出状态")?;
    if let Err(error) = std::fs::remove_file(path) {
        log::warn!("[updater] 无法清理更新缓存：{error}");
    }
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("安装向导异常退出：{status}"),
    }
}

fn lock_verified_package(path: &Path, verified_bytes: &[u8]) -> Result<File> {
    // Keep the verified file open without write or delete sharing through CreateProcess.
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .context("无法锁定已验证的安装包")?;
    let mut hash = Sha256::new();
    std::io::copy(&mut file, &mut hash).context("无法校验本地安装包")?;
    ensure!(
        hash.finalize() == Sha256::digest(verified_bytes),
        "保存后的安装包内容已变化"
    );
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_current_download_can_be_cancelled_before_handoff() {
        let controller = Controller::default();
        let (guard, mut cancelled) = controller.begin_download(7);
        assert!(!controller.cancel(6));
        assert!(controller.cancel(7));
        assert_eq!(cancelled.try_recv(), Ok(()));
        assert!(!guard.commit());
        drop(guard);

        let (guard, _) = controller.begin_download(8);
        assert!(guard.commit());
        assert!(!controller.cancel(8));
    }

    #[test]
    fn verified_package_can_start_while_replacement_is_blocked() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("update-fixture.exe");
        let command = std::env::var_os("COMSPEC").context("missing Windows command processor")?;
        let bytes = std::fs::read(command)?;
        std::fs::write(&path, &bytes)?;
        assert!(lock_verified_package(&path, b"changed bytes").is_err());
        let locked = lock_verified_package(&path, &bytes)?;
        assert!(OpenOptions::new().write(true).open(&path).is_err());
        assert!(
            Command::new(&path)
                .args(["/D", "/C", "exit", "0"])
                .status()?
                .success()
        );
        drop(locked);
        Ok(())
    }
}
