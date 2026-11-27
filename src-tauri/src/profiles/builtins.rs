//! 内置场景目录与本机目标解析。硬件应用沿用档案事务。
use super::{
    Devices, ManualPreview, ProfileFile, ProfileSettings, application, apply::review::Reviewed,
};
use crate::{
    display::{
        self, automatic,
        native::{EmbeddedPanel, embedded_panel},
    },
    gpuctl::GpuController,
    hal::Hal,
    powerctl::mode,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Id {
    QuietOffice,
    MobileBattery,
    PluggedGaming,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    id: Id,
    name: &'static str,
    description: &'static str,
    #[serde(skip)]
    requires_ac: bool,
    #[serde(skip)]
    mode: u8,
}

const TEMPLATES: &[Template] = &[
    Template {
        id: Id::QuietOffice,
        name: "安静办公",
        description: "会议、文档和浏览",
        requires_ac: false,
        mode: 0,
    },
    Template {
        id: Id::MobileBattery,
        name: "移动续航",
        description: "外出时降低耗电",
        requires_ac: false,
        mode: 1,
    },
    Template {
        id: Id::PluggedGaming,
        name: "插电游戏",
        description: "接入符合机型要求的适配器后运行高负载游戏",
        requires_ac: true,
        mode: 2,
    },
];

impl Id {
    fn template(self) -> &'static Template {
        TEMPLATES
            .iter()
            .find(|template| template.id == self)
            .expect("内置档案已登记")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    id: Id,
    #[serde(deserialize_with = "Option::deserialize")]
    refresh: Option<RefreshChoice>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RefreshChoice {
    revision: String,
    office_hz: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshOptions {
    revision: String,
    current: u32,
    candidates: Vec<u32>,
    low: u32,
    high: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Catalog {
    profiles: &'static [Template],
    refresh: Option<RefreshOptions>,
    refresh_issue: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct Preview {
    file: ProfileFile,
    review: ManualPreview,
}

pub(crate) fn catalog(app: &AppHandle) -> Catalog {
    let (refresh, refresh_issue) = match Panel::read(app) {
        Ok(panel) => (Some(panel.options), None),
        Err(error) => (None, Some(format!("{error:#}"))),
    };
    Catalog {
        profiles: TEMPLATES,
        refresh,
        refresh_issue,
    }
}

pub(crate) fn preview(app: &AppHandle, request: &Request) -> Result<Preview> {
    let resolved = Resolved::read(app, request)?;
    let mut review = super::preview_manual(
        &app.state::<Hal>(),
        &app.state::<GpuController>(),
        Devices::from_app(app),
        &resolved.file.settings,
    )?;
    if let Err(error) = validate_power(request.id) {
        review.blocked = Some(format!("{error:#}"));
    }
    Ok(Preview {
        file: resolved.file,
        review,
    })
}

pub(crate) fn apply(
    app: AppHandle,
    request: &Request,
    expected_file: &ProfileFile,
    expected_mode: Option<&mode::Target>,
    expected_display: Option<&display::profile::Review>,
) -> Result<application::Application> {
    let resolved = Resolved::read(&app, request)?;
    ensure!(
        resolved.file == *expected_file,
        "内置档案目标已变化，请重新预览"
    );
    let context = app.clone();
    let id = request.id;
    let source = application::Source::BuiltIn(Arc::new(move || {
        validate_power(id)?;
        if let Some(panel) = &resolved.panel {
            ensure_refresh_free(&context)?;
            ensure!(embedded_panel()? == *panel, "内屏身份已变化，请重新预览");
        }
        Ok(())
    }));
    app.state::<application::Controller>().begin_reviewed(
        application::Origin::Manual,
        &app.state::<Hal>(),
        &app.state::<GpuController>(),
        Devices::from_app(&app),
        Reviewed {
            permission: None,
            source: &source,
            file: &resolved.file,
            mode: expected_mode,
            display: expected_display,
            gpu_power: None,
            graphics_mode: None,
        },
    )
}

fn validate_power(id: Id) -> Result<()> {
    if id.template().requires_ac {
        require_ac(
            crate::battery::system_power()
                .context("无法确认插电状态")?
                .ac_online,
        )?;
    }
    Ok(())
}

fn require_ac(ac: Option<bool>) -> Result<()> {
    match ac {
        Some(true) => Ok(()),
        Some(false) => bail!("当前使用电池供电，请接入满足机型要求的电源适配器后重新预览"),
        None => bail!("供电状态未知，请确认适配器已接入后重新预览"),
    }
}

fn ensure_refresh_free(app: &AppHandle) -> Result<()> {
    ensure!(
        !app.state::<automatic::Controller>().owns_refresh()?,
        "内屏刷新率由「自动刷新率」管理"
    );
    Ok(())
}

struct Panel {
    embedded: EmbeddedPanel,
    mode: display::Mode,
    options: RefreshOptions,
}

impl Panel {
    fn read(app: &AppHandle) -> Result<Self> {
        ensure_refresh_free(app)?;
        let embedded = embedded_panel()?;
        let display = app
            .state::<display::Controller>()
            .read()?
            .displays
            .into_iter()
            .find(|display| {
                display
                    .current
                    .device
                    .eq_ignore_ascii_case(&embedded.device)
            })
            .context("未读取到活动内屏的显示模式")?;
        let mode = display.current;
        mode.validate()?;
        let candidates = display.frequencies;
        let [low, .., high] = candidates.as_slice() else {
            bail!("内屏需要至少两个可用刷新率")
        };
        ensure!(
            candidates.contains(&mode.frequency),
            "当前内屏刷新率不在已验证候选中"
        );
        let mut stable = mode.clone();
        stable.frequency = 0;
        stable.name.clear();
        let revision = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(&embedded, stable, &candidates))?)
        );
        let options = RefreshOptions {
            revision,
            current: mode.frequency,
            low: *low,
            high: *high,
            candidates,
        };
        Ok(Self {
            embedded,
            mode,
            options,
        })
    }
}

impl RefreshOptions {
    fn target(&self, id: Id, choice: &RefreshChoice) -> Result<u32> {
        ensure!(
            choice.revision == self.revision,
            "内屏身份、布局或候选已变化，请重新读取刷新率"
        );
        ensure!(
            self.candidates.contains(&choice.office_hz),
            "办公刷新率不在内屏候选中"
        );
        Ok(match id {
            Id::QuietOffice => choice.office_hz,
            Id::MobileBattery => self.low,
            Id::PluggedGaming => self.high,
        })
    }
}

struct Resolved {
    file: ProfileFile,
    panel: Option<EmbeddedPanel>,
}

impl Resolved {
    fn read(app: &AppHandle, request: &Request) -> Result<Self> {
        let template = request.id.template();
        let mut settings = ProfileSettings {
            oem_power_mode: Some(template.mode),
            ..Default::default()
        };
        let panel = if let Some(choice) = &request.refresh {
            let mut panel = Panel::read(app)?;
            panel.mode.frequency = panel.options.target(request.id, choice)?;
            settings.display = Some(display::profile::Target {
                displays: vec![panel.mode],
            });
            Some(panel.embedded)
        } else {
            None
        };
        let file = ProfileFile {
            version: 1,
            name: template.name.into(),
            saved_at_ms: 0,
            settings,
        };
        super::validate_file(&file)?;
        Ok(Self { file, panel })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_modes_and_ac_requirement_match_the_three_scenarios() {
        let values: Vec<_> = TEMPLATES
            .iter()
            .map(|template| (template.name, template.mode, template.requires_ac))
            .collect();
        assert_eq!(
            values,
            [
                ("安静办公", 0, false),
                ("移动续航", 1, false),
                ("插电游戏", 2, true)
            ]
        );
        assert!(require_ac(Some(true)).is_ok());
        assert!(require_ac(Some(false)).is_err());
        assert!(require_ac(None).is_err());
    }

    #[test]
    fn refresh_choices_resolve_all_scenarios_and_reject_stale_or_invalid_office_targets() {
        let options = RefreshOptions {
            revision: "panel".into(),
            current: 120,
            candidates: vec![60, 120, 165],
            low: 60,
            high: 165,
        };
        let mut choice = RefreshChoice {
            revision: "panel".into(),
            office_hz: 120,
        };
        assert_eq!(options.target(Id::QuietOffice, &choice).unwrap(), 120);
        assert_eq!(options.target(Id::MobileBattery, &choice).unwrap(), 60);
        assert_eq!(options.target(Id::PluggedGaming, &choice).unwrap(), 165);
        choice.office_hz = 144;
        assert!(options.target(Id::MobileBattery, &choice).is_err());
        choice.office_hz = 120;
        choice.revision = "old panel".into();
        assert!(options.target(Id::PluggedGaming, &choice).is_err());
    }
}
