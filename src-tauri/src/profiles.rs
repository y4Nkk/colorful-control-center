//! 档案磁盘入口；数据与校验由 profiles/model.rs 拥有，执行事务见 apply.rs。
pub mod application;
mod apply;
pub(crate) mod authorization;
mod capture;
mod devices;
mod model;
pub mod policy;
mod preview;
use anyhow::{Context, Result, anyhow, bail};
pub(crate) use application::apply_startup;
pub use application::{Application, apply_preview};
#[cfg(test)]
pub(crate) use apply::apply_reviewed;
pub(crate) use apply::review::{ManualPreview, preview_manual};
pub(crate) use devices::Devices;
pub(crate) use preview::preview_authorization;
#[cfg(test)]
pub(crate) use preview::preview_mode;
use std::path::{Path, PathBuf};
use tauri::Manager;
static PROFILE_MUTATION: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub use model::{ApplyStep, ProfileFile, ProfileGroup, ProfileMeta, ProfileSettings, validate};

pub fn sanitize_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty()
        || name.chars().count() > 60
        || name.ends_with('.')
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
    {
        bail!("档案名须为 1–60 个文字、数字或 - _ .，不能以点结尾");
    }
    let reserved = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "COM¹",
        "COM²", "COM³", "LPT¹", "LPT²", "LPT³",
    ]
    .contains(&reserved.as_str())
    {
        bail!("此档案名为系统保留名称");
    }
    Ok(name.into())
}

pub fn validate_file(file: &ProfileFile) -> Result<()> {
    if file.version != 1 {
        bail!("不支持的档案版本 {}", file.version);
    }
    if file.saved_at_ms > 8_640_000_000_000_000 {
        bail!("档案时间戳超出界面可表示范围");
    }
    sanitize_name(&file.name)?;
    validate(&file.settings)
}

pub(crate) fn profiles_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf> {
    let dir = app.path().app_data_dir()?.join("profiles");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn profile_path(dir: &Path, name: &str) -> Result<PathBuf> {
    Ok(dir.join(format!("{}.json", sanitize_name(name)?)))
}

pub(crate) fn same_name(left: &str, right: &str) -> bool {
    let left: Vec<_> = left.encode_utf16().collect();
    let right: Vec<_> = right.encode_utf16().collect();
    let (Ok(left_len), Ok(right_len)) = (i32::try_from(left.len()), i32::try_from(right.len()))
    else {
        return false;
    };
    unsafe {
        windows_sys::Win32::Globalization::CompareStringOrdinal(
            left.as_ptr(),
            left_len,
            right.as_ptr(),
            right_len,
            1,
        ) == windows_sys::Win32::Globalization::CSTR_EQUAL
    }
}

fn check_existing_spelling(dir: &Path, path: &Path) -> Result<()> {
    let expected = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("档案文件名无效"))?;
    for entry in std::fs::read_dir(dir)? {
        let name = entry?.file_name();
        if let Some(name) = name.to_str() {
            if same_name(name, expected) && name != expected {
                bail!("档案名称与已有文件仅大小写不同，请使用已有名称或选择其他名称");
            }
        }
    }
    Ok(())
}

pub(crate) fn save(
    app: &tauri::AppHandle,
    settings: ProfileSettings,
    name: &str,
    expected_revision: Option<&str>,
) -> Result<()> {
    save_in(&profiles_dir(app)?, settings, name, expected_revision)
}

fn save_in(
    dir: &Path,
    settings: ProfileSettings,
    name: &str,
    expected_revision: Option<&str>,
) -> Result<()> {
    let _lock = PROFILE_MUTATION
        .lock()
        .map_err(|_| anyhow!("档案存储状态异常"))?;
    validate(&settings)?;
    let path = profile_path(dir, name)?;
    let file = ProfileFile {
        version: 1,
        name: sanitize_name(name)?,
        saved_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64,
        settings,
    };
    std::fs::create_dir_all(dir)?;
    check_existing_spelling(dir, &path)?;
    check_revision(&path, &file.name, expected_revision)?;
    crate::storage::write(&path, &serde_json::to_vec_pretty(&file)?).context("保存档案失败")
}

fn check_revision(path: &Path, name: &str, expected: Option<&str>) -> Result<()> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let Some(expected) = expected else {
                bail!("已存在同名档案，请重新读取并确认覆盖");
            };
            let file: ProfileFile = serde_json::from_slice(&bytes).context("当前档案损坏")?;
            validate_file(&file)?;
            if file.name != name || authorization::digest(&file)? != expected {
                bail!("档案已变化，请重新读取并确认");
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if expected.is_some() {
                bail!("档案已移除，请重新读取");
            }
            Ok(())
        }
        Err(error) => Err(error).context("核对档案修订失败"),
    }
}

pub fn list(app: &tauri::AppHandle) -> Result<Vec<ProfileMeta>> {
    list_in(&profiles_dir(app)?)
}

fn list_in(dir: &Path) -> Result<Vec<ProfileMeta>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let data = std::fs::read(&path).with_context(|| format!("读取 {} 失败", path.display()))?;
        let file: ProfileFile = serde_json::from_slice(&data)
            .with_context(|| format!("档案 {} 损坏", path.display()))?;
        validate_file(&file)?;
        if profile_path(dir, &file.name)? != path {
            bail!("档案名与文件名不一致");
        }
        let revision = authorization::digest(&file)?;
        files.push(ProfileMeta {
            groups: ProfileGroup::captured(&file.settings),
            name: file.name,
            revision,
        });
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

pub fn load(app: &tauri::AppHandle, name: &str) -> Result<ProfileFile> {
    load_from(&profiles_dir(app)?, name)
}

fn load_from(dir: &Path, name: &str) -> Result<ProfileFile> {
    let file: ProfileFile = serde_json::from_slice(&std::fs::read(profile_path(dir, name)?)?)?;
    validate_file(&file)?;
    if file.name != sanitize_name(name)? {
        bail!("档案名与文件名不一致");
    }
    Ok(file)
}

pub(crate) fn delete(app: &tauri::AppHandle, name: &str, expected_revision: &str) -> Result<bool> {
    // 删除先锁档案，再解除启动引用；启动选择的只读校验不取档案写锁。
    let _lock = PROFILE_MUTATION
        .lock()
        .map_err(|_| anyhow!("档案存储状态异常"))?;
    let path = profile_path(&profiles_dir(app)?, name)?;
    check_revision(&path, name, Some(expected_revision))?;
    crate::startup::delete_linked_profile(app, name, || {
        std::fs::remove_file(path).context("删除档案文件失败")
    })
}

pub(crate) use capture::capture_current;

#[cfg(test)]
#[path = "profiles/storage_tests.rs"]
mod storage_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_timestamp_roundtrips_through_javascript_preview() {
        let mut file = ProfileFile {
            version: 1,
            name: "测试".into(),
            saved_at_ms: 8_640_000_000_000_000,
            settings: ProfileSettings {
                fan_mode: Some(0),
                ..Default::default()
            },
        };
        assert!(validate_file(&file).is_ok());
        file.saved_at_ms += 1;
        assert!(validate_file(&file).is_err());
        file.saved_at_ms = u64::MAX;
        assert!(validate_file(&file).is_err());
    }
    #[test]
    fn rejects_reserved_or_path_names() {
        for name in [
            "../x",
            "CON",
            "LPT1.json",
            "foo.",
            "",
            "COM¹",
            "com².json",
            "COM³",
            "LPT¹.json",
            "lpt²",
            "LPT³",
        ] {
            assert!(sanitize_name(name).is_err(), "{name}");
        }
        assert_eq!(sanitize_name(" 静音 ").unwrap(), "静音");
        assert_eq!(sanitize_name("静音¹").unwrap(), "静音¹");
        assert_eq!(sanitize_name("COM10").unwrap(), "COM10");
    }
    #[test]
    fn rejects_wrong_version_and_unknown_settings() {
        let file = ProfileFile {
            version: 2,
            name: "测试".into(),
            saved_at_ms: 0,
            settings: ProfileSettings {
                fan_mode: Some(0),
                ..Default::default()
            },
        };
        assert!(validate_file(&file).is_err());
        assert!(serde_json::from_str::<ProfileSettings>(r#"{"obsolete":true}"#).is_err());
    }
}
