//! 同目录准备完整内容后替换；提交前不会打开或截断目标文件。
use std::{
    io::{self, Write},
    path::Path,
};
use tempfile::NamedTempFile;

pub fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut staged = stage(path, contents)?;
    let mut retries = 8;
    loop {
        match staged.persist(path) {
            Ok(_) => return Ok(()),
            Err(error) if retries > 0 && matches!(error.error.raw_os_error(), Some(5 | 32)) => {
                retries -= 1;
                staged = error.file;
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(error) => return Err(error.error),
        }
    }
}

pub(super) fn stage(path: &Path, contents: &[u8]) -> io::Result<NamedTempFile> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "目标文件缺少父目录"))?;
    let mut file = tempfile::Builder::new()
        .prefix(".ccc-")
        .suffix(".tmp")
        .tempfile_in(parent)?;
    file.write_all(contents)?;
    file.as_file().sync_all()?;
    Ok(file)
}

#[cfg(test)]
#[path = "storage/tests.rs"]
mod tests;
