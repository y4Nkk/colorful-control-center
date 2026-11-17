//! 只复用生产恢复记录；不链接HAL或加载任何硬件接口。
#[path = "../../../../src-tauri/src/hal/fan_safety/journal.rs"]
mod journal;
#[path = "../../../../src-tauri/src/storage.rs"]
mod storage;
use anyhow::{Context, Result, bail, ensure};
use std::{fs, io, path::PathBuf};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(args.len() == 2, "需要隔离目录和场景");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/fan-safety-tests")
        .canonicalize()?;
    let dir = PathBuf::from(&args[0]).canonicalize()?;
    ensure!(
        dir.starts_with(&root) && dir != root,
        "目录不在专用隔离范围内"
    );
    let mut journal = journal::Journal::open(dir.join(journal::RECORD_NAME));
    match args[1].to_str().context("场景不是UTF-8")? {
        "armed" => {
            journal.begin(false)?;
            fs::write(dir.join("ready"), b"armed")?;
            let mut line = String::new();
            io::stdin().read_line(&mut line)?;
            bail!("父进程未强杀隔离写入者");
        }
        "fault" => {
            journal.begin(false)?;
            journal.latch()?;
            journal.stopped()?;
        }
        "clean" => {
            journal.begin(false)?;
            journal.close();
            journal.stopped()?;
        }
        "confirmed" => {
            journal.begin(true)?;
            journal.acknowledge()?;
            journal.stopped()?;
        }
        "check" => journal.allowed()?,
        _ => bail!("未知隔离场景"),
    }
    Ok(())
}
