use super::{
    AppHandle, Manager, Result, actions, cancelled, keymap, low_battery, mpsc, osd, publish,
    report, store,
};
use crate::hal::power_events::{Event, Source as PowerSource};
use crate::hotkeys::battery_cold::{self, Trigger, Warning};
use crate::hotkeys::model::{Record, Scope, Status};
use anyhow::{Context, anyhow, bail};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Input {
    Startup,
    Resume,
    Ac,
}
pub(super) fn prepare(record: &Record) -> Result<(crate::hal::event_log::Log, Source)> {
    let log = crate::hal::event_log::Log::open("OutLog")?;
    let source = Source::prepare(
        record.settings.actions.contains(&Scope::BatteryColdWarning),
        record
            .settings
            .actions
            .iter()
            .any(|scope| matches!(scope, Scope::BatteryLowBrightness | Scope::BatteryAlert)),
    )?;
    Ok((log, source))
}
fn classify(event: Event) -> Option<Input> {
    match event {
        Event::Resume => Some(Input::Resume),
        Event::Source(PowerSource::Ac) => Some(Input::Ac),
        _ => None,
    }
}
pub(super) struct Source {
    enabled: bool,
    cold_enabled: bool,
    low_enabled: bool,
    startup: bool,
    #[cfg(not(test))]
    _listener: Option<crate::hal::power_events::Session>,
    receiver: mpsc::Receiver<std::result::Result<Input, String>>,
    #[cfg(test)]
    sender: mpsc::Sender<std::result::Result<Input, String>>,
}
impl Source {
    pub fn prepare(cold_enabled: bool, low_enabled: bool) -> Result<Self> {
        let enabled = cold_enabled || low_enabled;
        let (sender, receiver) = mpsc::channel();
        #[cfg(not(test))]
        let listener = if enabled {
            let events = sender.clone();
            let failures = sender.clone();
            let session = crate::hal::power_events::Session::start(
                crate::hal::power_events::DisplayPolicy::Unknown,
                move |event| {
                    if let Some(input) = classify(event) {
                        events.send(Ok(input)).context("发送低温电源事件")?;
                    }
                    Ok(())
                },
                move |error| {
                    let _ = failures.send(Err(error));
                },
            )?;
            session.status()?;
            Some(session)
        } else {
            None
        };
        Ok(Self {
            enabled,
            cold_enabled,
            low_enabled,
            startup: enabled,
            receiver,
            #[cfg(not(test))]
            _listener: listener,
            #[cfg(test)]
            sender,
        })
    }
    fn next(&mut self) -> Result<Option<Input>> {
        if self.startup {
            self.startup = false;
            return Ok(Some(Input::Startup));
        }
        if !self.enabled {
            return Ok(None);
        }
        match self.receiver.try_recv() {
            Ok(Ok(input)) => Ok(Some(input)),
            Ok(Err(error)) => bail!("电池事件的电源通知失败：{error}"),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => bail!("电池事件的电源通知已断开"),
        }
    }
}
fn clear(status: &Mutex<Status>) -> Result<bool> {
    let mut value = status.lock().map_err(|_| anyhow!("热键运行状态异常"))?;
    Ok(value.cold_warning.take().is_some())
}
fn clear_on_power(status: &Mutex<Status>, online: Option<bool>) -> Result<bool> {
    match online {
        Some(true) => clear(status),
        Some(false) => Ok(false),
        None => bail!("供电状态未知，保留低温提醒"),
    }
}
fn set(status: &Mutex<Status>, temperature: i32, percent: u8) -> Result<u64> {
    let mut value = status.lock().map_err(|_| anyhow!("热键运行状态异常"))?;
    value.warning_sequence = value
        .warning_sequence
        .checked_add(1)
        .context("低温提醒序号耗尽")?;
    value.cold_warning = Some(Warning {
        id: value.warning_sequence,
        temperature_c: temperature,
        battery_percent: percent,
    });
    value.last_result = Some(format!(
        "电池低温：{temperature}°C，电量 {percent}%；请接入电源"
    ));
    value.error = None;
    Ok(value.warning_sequence)
}
fn still_current(status: &Mutex<Status>, expected: u64) -> Result<bool> {
    Ok(status
        .lock()
        .map_err(|_| anyhow!("热键运行状态异常"))?
        .cold_warning
        .as_ref()
        .is_some_and(|warning| warning.id == expected))
}
fn show_main(app: &AppHandle, status: &Arc<Mutex<Status>>, expected: u64) -> Result<()> {
    let handle = app.clone();
    let status = status.clone();
    app.run_on_main_thread(move || {
        let result = (|| -> Result<()> {
            if !still_current(&status, expected)? {
                return Ok(());
            }
            let window = handle.get_webview_window("main").context("主窗口不可用")?;
            window.show().context("显示主窗口")?;
            window.unminimize().context("恢复主窗口")?;
            window.set_focus().context("聚焦主窗口")?;
            Ok(())
        })();
        if let Err(error) = result {
            if let Ok(mut value) = status.lock() {
                if value
                    .cold_warning
                    .as_ref()
                    .is_some_and(|warning| warning.id == expected)
                {
                    value.error = Some(format!("低温提醒窗口显示或聚焦失败：{error:#}"));
                }
            }
            log::warn!("[hotkeys] 低温提醒窗口：{error:#}");
            publish(&handle, &status);
        }
    })
    .context("请求显示低温提醒窗口")
}
pub(super) fn handle(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &Record,
    status: &Arc<Mutex<Status>>,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
    trigger: Trigger,
) -> Result<bool> {
    let result = battery_cold::read(&app.state::<crate::hal::Hal>(), identity, trigger);
    if cancelled(stopping) {
        return Ok(true);
    }
    if store::load(path)?.record != *approved {
        bail!("低温提醒策略许可已撤销");
    }
    approved.verify_action(
        &actions::facts(app, &[Scope::BatteryColdWarning]),
        Scope::BatteryColdWarning,
    )?;
    match result {
        Ok(Some((temperature, percent))) => {
            let id = set(status, temperature, percent)?;
            publish(app, status);
            show_main(app, status, id)?;
        }
        Ok(None) => (),
        Err(error) => {
            let mut message = osd::Message::result("低温提醒不可用", format!("{error:#}"));
            message.error = true;
            report(
                app,
                status,
                ccc_core::oem_events::Action::BatteryColdWarning,
                false,
                message,
            );
        }
    }
    Ok(false)
}
fn update_for_ac(app: &AppHandle, status: &Mutex<Status>) -> Result<()> {
    let has_warning = {
        status
            .lock()
            .map_err(|_| anyhow!("热键运行状态异常"))?
            .cold_warning
            .is_some()
    };
    if !has_warning {
        return Ok(());
    }
    match crate::battery::system_power().and_then(|power| clear_on_power(status, power.ac_online)) {
        Ok(true) => publish(app, status),
        Ok(false) => (),
        Err(error) => {
            let mut message = osd::Message::result("低温提醒状态不可用", format!("{error:#}"));
            message.error = true;
            report(
                app,
                status,
                ccc_core::oem_events::Action::BatteryColdWarning,
                false,
                message,
            );
        }
    }
    Ok(())
}
pub(super) fn tick(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &Record,
    status: &Arc<Mutex<Status>>,
    stopping: &mpsc::Receiver<()>,
    source: &mut Source,
) -> Result<bool> {
    while let Some(input) = source.next()? {
        if cancelled(stopping) {
            return Ok(true);
        }
        if input == Input::Ac && source.cold_enabled {
            update_for_ac(app, status)?;
        }
        if input == Input::Ac && !source.low_enabled {
            continue;
        }
        if app.state::<keymap::Controller>().recording_active()? {
            continue;
        }
        if store::load(path)?.record != *approved {
            bail!("低温提醒策略文件已变化");
        }
        let brightness_scope = if approved
            .settings
            .actions
            .contains(&Scope::BatteryLowBrightness)
        {
            Scope::BatteryLowBrightness
        } else {
            Scope::BatteryAlert
        };
        let scopes: Vec<_> = [
            (Scope::BatteryColdWarning, source.cold_enabled),
            (brightness_scope, source.low_enabled),
        ]
        .into_iter()
        .filter_map(|(scope, enabled)| enabled.then_some(scope))
        .collect();
        let facts = actions::facts(app, &scopes);
        if input != Input::Ac && source.cold_enabled {
            approved.verify_action(&facts, Scope::BatteryColdWarning)?;
            let identity = identity(&facts, Scope::BatteryColdWarning)?;
            let trigger = if input == Input::Startup {
                Trigger::Startup
            } else {
                Trigger::Resume
            };
            if handle(app, path, approved, status, stopping, identity, trigger)? {
                return Ok(true);
            }
        }
        if source.low_enabled {
            approved.verify_action(&facts, brightness_scope)?;
            let brightness = if brightness_scope == Scope::BatteryAlert {
                facts
                    .battery_alert
                    .as_ref()
                    .and_then(|profile| profile.binding.as_ref())
                    .filter(|binding| binding.low_control)
                    .map(|binding| binding.brightness.as_str())
            } else {
                Some(identity(&facts, brightness_scope)?)
            };
            if let Some(identity) = brightness {
                if low_battery::check(app, path, approved, status, stopping, identity)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
fn identity(facts: &crate::hotkeys::model::Facts, scope: Scope) -> Result<&str> {
    facts
        .capabilities
        .iter()
        .find(|cap| cap.scope == scope)
        .and_then(|cap| cap.identity.as_deref())
        .context("电池事件能力不可用")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_resume_and_known_ac_are_dispatched_and_startup_is_once() {
        assert_eq!(classify(Event::Resume), Some(Input::Resume));
        assert_eq!(classify(Event::Source(PowerSource::Ac)), Some(Input::Ac));
        for event in [
            Event::Suspend,
            Event::Source(PowerSource::Battery),
            Event::Source(PowerSource::Unknown),
        ] {
            assert_eq!(classify(event), None);
        }
        let mut source = Source::prepare(true, false).unwrap();
        assert_eq!(source.next().unwrap(), Some(Input::Startup));
        assert_eq!(source.next().unwrap(), None);
        source.sender.send(Ok(Input::Resume)).unwrap();
        assert_eq!(source.next().unwrap(), Some(Input::Resume));
        source.sender.send(Err("隔离故障".into())).unwrap();
        assert!(source.next().is_err());
        let mut low_only = Source::prepare(false, true).unwrap();
        assert_eq!(low_only.next().unwrap(), Some(Input::Startup));
        let mut disabled = Source::prepare(false, false).unwrap();
        assert_eq!(disabled.next().unwrap(), None);
    }
    #[test]
    fn warning_ids_replace_old_warning_and_ac_clears_only_warning() {
        let status = Mutex::new(Status::default());
        set(&status, 10, 80).unwrap();
        let first = status.lock().unwrap().cold_warning.clone().unwrap();
        assert!(still_current(&status, first.id).unwrap());
        set(&status, -2, 50).unwrap();
        let second = status.lock().unwrap().cold_warning.clone().unwrap();
        assert!(second.id > first.id);
        assert!(!still_current(&status, first.id).unwrap());
        assert!(
            status
                .lock()
                .unwrap()
                .dismiss_cold_warning(first.id)
                .is_err()
        );
        assert_eq!(
            status.lock().unwrap().cold_warning.as_ref().unwrap().id,
            second.id
        );
        status
            .lock()
            .unwrap()
            .dismiss_cold_warning(second.id)
            .unwrap();
        set(&status, -1, 49).unwrap();
        assert!(!clear_on_power(&status, Some(false)).unwrap());
        assert!(clear_on_power(&status, None).is_err());
        assert!(clear_on_power(&status, Some(true)).unwrap());
        assert!(!clear_on_power(&status, Some(true)).unwrap());
        assert!(status.lock().unwrap().last_result.is_some());
    }
}
