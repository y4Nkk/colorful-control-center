use super::*;

pub(super) fn brightness(app: &AppHandle, expected: &str, delta: i8) -> Result<osd::Message> {
    let hal = app.state::<Hal>();
    let (value, max) = if let Some(identity) = expected.strip_prefix("perkey:") {
        (
            app.state::<lighting::perkey::Controller>()
                .adjust_brightness(&hal, identity, delta)?,
            4,
        )
    } else if let Some(expected) = expected.strip_prefix("multiZone:") {
        (
            app.state::<lighting::multizone::Controller>()
                .adjust_brightness(&hal, expected, delta)?,
            4,
        )
    } else {
        let controller = app.state::<lighting::LightingController>();
        let level = controller.adjust_brightness(&hal, expected, delta)?;
        (level, if expected.starts_with("White:") { 5 } else { 4 })
    };
    let mut message = osd::Message::result("键盘亮度", "设置已发送并保存");
    message.level = Some(osd::Level { value, max });
    Ok(message)
}

pub(super) fn brightness_event(app: &AppHandle, expected: &str) -> Result<Completion> {
    let report = app
        .state::<lighting::LightingController>()
        .brightness_event(&app.state::<Hal>(), expected)?;
    Ok(brightness_completion(report))
}
pub(in crate::hotkeys) fn brightness_completion(
    report: Option<lighting::brightness_event::Report>,
) -> Completion {
    let Some(report) = report else {
        return Completion {
            message: None,
            show_osd: false,
        };
    };
    let mut message = osd::Message::result(
        "键盘亮度",
        if report.firmware_report {
            format!(
                "固件报告第 {} 档已同步到保存值，未发送亮度命令",
                report.level
            )
        } else {
            "设置已发送并保存".into()
        },
    );
    if report.show_osd() {
        message.level = Some(osd::Level {
            value: report.level,
            max: report.display_maximum,
        });
    }
    Completion {
        message: Some(message),
        show_osd: report.show_osd(),
    }
}

pub(super) fn keyboard_switch(
    app: &AppHandle,
    expected: &str,
    rgb_only: bool,
) -> Result<osd::Message> {
    let hal = app.state::<Hal>();
    if rgb_only {
        let raw = app
            .state::<lighting::LightingController>()
            .read_profile(&hal)
            .detected_type
            .context("键盘型号未知，未执行热键")?;
        if !ccc_core::oem_events::keyboard_toggle_type_allowed(rgb_only, raw) {
            bail!("此原厂开关事件不适用于当前键盘型号");
        }
    }
    let enabled = if let Some(identity) = expected.strip_prefix("perkey:") {
        app.state::<lighting::perkey::Controller>()
            .toggle_enabled(&hal, identity)?
    } else if let Some(identity) = expected.strip_prefix("multiZone:") {
        app.state::<lighting::multizone::Controller>()
            .toggle_enabled(&hal, identity)?
    } else {
        app.state::<lighting::LightingController>()
            .toggle_enabled(&hal, expected)?
    };
    Ok(osd::Message::result(
        "键盘灯",
        if enabled {
            "已开启，保留原亮度"
        } else {
            "已关闭，亮度和灯效已保留"
        },
    ))
}

pub(super) fn keyboard_zone(app: &AppHandle, expected: &str, zone: u8) -> Result<osd::Message> {
    let title = ["键盘左区", "键盘中区", "键盘右区"]
        .get(usize::from(zone))
        .context("未知键盘分区")?;
    let enabled = app.state::<lighting::LightingController>().toggle_zone(
        &app.state::<Hal>(),
        expected,
        zone,
    )?;
    Ok(osd::Message::result(
        *title,
        if enabled {
            "已开启，保留键盘总开关"
        } else {
            "已关闭"
        },
    ))
}

pub(super) fn keyboard_color(app: &AppHandle, expected: &str) -> Result<osd::Message> {
    let choice = app
        .state::<lighting::LightingController>()
        .cycle_color(&app.state::<Hal>(), expected)?;
    Ok(osd::Message::result("键盘颜色", choice.summary()))
}

pub(super) fn fn_lock(app: &AppHandle, expected: &str) -> Result<osd::Message> {
    let hal = app.state::<Hal>();
    require("fn-lock-supported".into(), expected)?;
    let controller = app.state::<keyboard::Controller>();
    let profile = controller.read(&hal)?;
    let locked = !profile.current.fn_locked.context("Fn锁能力不可用")?;
    let mut values = profile.current;
    values.fn_locked = Some(locked);
    controller.apply(
        &hal,
        &keyboard::Request {
            expected_revision: profile.revision,
            values,
        },
    )?;
    Ok(osd::Message::result(
        "Fn 锁",
        if locked { "已开启" } else { "已关闭" },
    ))
}
