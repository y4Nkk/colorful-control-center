//! 开机档案：显式 --autostart 启动时仅应用文件与模式授权仍匹配的已确认档案。
use crate::profiles::application::describe_application;
mod authorization;
mod desktop;
mod instance;
mod launch;
pub(crate) mod login;
mod oem;
mod registry;
mod restart;
use crate::profiles::authorization::Binding;
use anyhow::{Context, Result, anyhow};
pub(crate) use desktop::{DesktopStart, prepare_desktop, report_start_failure};
pub(crate) use instance::stop_activation;
pub use launch::{LaunchReport, apply_login_profile};
pub(crate) use launch::{preparing, profile_in_progress};
pub(crate) use oem::{OemStartupNotice, close_oem_at_launch, start_oem_worker};
pub(crate) use restart::{RestartMode, request_restart};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::Manager;

static PREFERENCE_MUTATION: Mutex<()> = Mutex::new(());
const PREFERENCE_VERSION: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Preferences {
    version: u8,
    #[serde(deserialize_with = "Option::deserialize")]
    profile: Option<Binding>,
}
impl Preferences {
    fn selected(profile: Option<Binding>) -> Self {
        Self {
            version: PREFERENCE_VERSION,
            profile,
        }
    }
    fn validate(&self) -> Result<()> {
        if self.version != PREFERENCE_VERSION {
            anyhow::bail!("启动档案设置版本已过期，请重新预览并保存启动选择");
        }
        if let Some(profile) = &self.profile {
            profile.validate()?;
        }
        Ok(())
    }
}
impl Default for Preferences {
    fn default() -> Self {
        Self::selected(None)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartupProfile {
    pub selection: Option<Binding>,
    pub binding_issue: Option<String>,
    pub last_run: Option<String>,
}

fn read(app: &tauri::AppHandle) -> Result<Preferences> {
    read_from(&app.path().app_data_dir()?)
}

fn read_from(dir: &std::path::Path) -> Result<Preferences> {
    let path = dir.join("startup-profile.json");
    match std::fs::read(path) {
        Ok(data) => {
            let preferences: Preferences = serde_json::from_slice(&data)
                .context("启动档案设置损坏或版本已过期，请重新预览并保存启动选择")?;
            preferences.validate()?;
            Ok(preferences)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn get_profile(app: &tauri::AppHandle) -> Result<StartupProfile> {
    let application = app
        .state::<crate::profiles::application::Controller>()
        .startup_report(&app.state::<crate::display::Controller>())?;
    let last_run = if let Some(result) = application {
        Some(describe_application(&result))
    } else {
        app.state::<LaunchReport>().last_error()?
    };
    let selection = read(app)?.profile;
    let dir = crate::profiles::profiles_dir(app)?;
    let binding_issue = selection
        .as_ref()
        .and_then(|binding| binding.load(&dir).err())
        .map(|error| format!("{error:#}"));
    Ok(StartupProfile {
        selection,
        binding_issue,
        last_run,
    })
}

pub(crate) fn set_profile(
    app: &tauri::AppHandle,
    expected: Option<crate::profiles::ProfileFile>,
    expected_mode: Option<&crate::powerctl::mode::profile::ApprovalPreview>,
) -> Result<Option<Binding>> {
    let Some(expected) = expected else {
        if expected_mode.is_some() {
            anyhow::bail!("取消启动选择不能携带模式授权");
        }
        let _lock = PREFERENCE_MUTATION
            .lock()
            .map_err(|_| anyhow!("启动档案配置状态异常"))?;
        write_to(&app.path().app_data_dir()?, &Preferences::default())?;
        return Ok(None);
    };
    let hal = app.state::<crate::hal::Hal>();
    let gpu = app.state::<crate::gpuctl::GpuController>();
    let power = app.state::<crate::gpu_power::Controller>();
    let session = crate::powerctl::mode::links::Context::native(&hal, &gpu, &power).session()?;
    let _gate = hal.begin_write()?;
    if crate::profiles::preview_authorization(&session.context, &expected.settings)?.as_ref()
        != expected_mode
    {
        anyhow::bail!("启动档案完整模式授权已变化，请重新预览");
    }
    let _lock = PREFERENCE_MUTATION
        .lock()
        .map_err(|_| anyhow!("启动档案配置状态异常"))?;
    let profile = reviewed_selection(Some(expected), expected_mode, |name| {
        crate::profiles::load(app, name)
    })?;
    write_to(
        &app.path().app_data_dir()?,
        &Preferences::selected(profile.clone()),
    )?;
    Ok(profile)
}

fn reviewed_selection(
    expected: Option<crate::profiles::ProfileFile>,
    mode: Option<&crate::powerctl::mode::profile::ApprovalPreview>,
    load: impl FnOnce(&str) -> Result<crate::profiles::ProfileFile>,
) -> Result<Option<Binding>> {
    let Some(expected) = expected else {
        return Ok(None);
    };
    let actual = load(&expected.name)?;
    if actual != expected {
        anyhow::bail!("启动档案已变化，请重新预览后确认");
    }
    Ok(Some(Binding::reviewed(
        &actual,
        mode.map(|mode| mode.grant()).transpose()?,
    )?))
}

fn write_to(dir: &std::path::Path, preferences: &Preferences) -> Result<()> {
    preferences.validate()?;
    std::fs::create_dir_all(dir)?;
    crate::storage::write(
        &dir.join("startup-profile.json"),
        &serde_json::to_vec(preferences)?,
    )
    .context("保存启动档案失败")
}

#[cfg(test)]
pub(crate) fn test_permission(
    dir: &std::path::Path,
    binding: Binding,
) -> Result<crate::profiles::application::Permission> {
    let preferences = Preferences::selected(Some(binding));
    write_to(dir, &preferences)?;
    Ok(authorization::permission(dir.to_owned(), preferences))
}

pub(crate) fn delete_linked_profile(
    app: &tauri::AppHandle,
    name: &str,
    delete: impl FnOnce() -> Result<()>,
) -> Result<bool> {
    unlink_from(&app.path().app_data_dir()?, name, delete)
}

fn unlink_from(
    dir: &std::path::Path,
    name: &str,
    delete: impl FnOnce() -> Result<()>,
) -> Result<bool> {
    let _lock = PREFERENCE_MUTATION
        .lock()
        .map_err(|_| anyhow!("启动档案配置状态异常"))?;
    let before = read_from(dir)?;
    let selected = before
        .profile
        .as_ref()
        .map(|binding| crate::profiles::sanitize_name(&binding.profile))
        .transpose()?;
    let linked = selected
        .as_deref()
        .is_some_and(|profile| crate::profiles::same_name(profile, name));
    if linked {
        write_to(dir, &Preferences::default())?;
    }
    if let Err(error) = delete() {
        if linked {
            if let Err(restore) = write_to(dir, &before) {
                return Err(anyhow!(
                    "{error:#}；启动选择恢复失败，当前已取消开机应用：{restore:#}"
                ));
            }
        }
        return Err(error.context("删除档案失败，启动选择保持原状态"));
    }
    Ok(linked)
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
