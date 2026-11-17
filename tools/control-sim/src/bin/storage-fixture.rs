//! 专用文件中断进程：只写调用者的隔离目录，不链接应用或 HAL。
#[path = "../../../../src-tauri/src/storage.rs"]
mod storage;
use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let root = PathBuf::from(args.get(1).context("缺少隔离目录")?);
    let mode = args.get(2).context("缺少写入场景")?;
    let target = root.join("档案.json");
    let bytes = br#"{"profile":"new","pl1W":75.125}"#;
    match mode.as_str() {
        "stage" => {
            let staged = storage::stage(&target, bytes)?;
            fs::write(
                root.join("staged"),
                staged.path().to_str().context("临时路径无效")?,
            )?;
            let deadline = Instant::now() + Duration::from_secs(8);
            while !root.join("release").exists() {
                if Instant::now() >= deadline {
                    bail!("隔离准备阶段超时");
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            drop(staged);
        }
        "commit" => storage::write(&target, bytes)?,
        _ => bail!("未知文件场景"),
    }
    Ok(())
}
