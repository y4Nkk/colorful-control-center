use crate::{
    hal::{Hal, HardwareSnapshot},
    shutdown, tray,
};
use std::{
    sync::{Condvar, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

const DISPLAY_INTERVAL: Duration = Duration::from_secs(1);
const TOOLTIP_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Demand {
    active: bool,
    revision: u64,
}

#[derive(Default)]
pub(crate) struct Controller {
    demand: Mutex<Demand>,
    changed: Condvar,
}

impl Controller {
    pub(crate) fn set_active(&self, active: bool) {
        let mut demand = self
            .demand
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        demand.active = active;
        demand.revision = demand.revision.wrapping_add(1);
        self.changed.notify_one();
    }

    fn demand(&self) -> Demand {
        *self
            .demand
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn wait(&self, previous: Demand, until: Instant) {
        let demand = self
            .demand
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        drop(self.changed.wait_timeout_while(
            demand,
            until.saturating_duration_since(Instant::now()),
            |current| *current == previous,
        ));
    }
}

pub(crate) fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || run(&app));
}

fn run(app: &AppHandle) {
    let controller = app.state::<Controller>();
    let hal = app.state::<Hal>();
    let mut previous = Demand::default();
    let mut display_at = Instant::now();
    let mut tooltip_at = display_at + DISPLAY_INTERVAL;
    let mut latest: Option<(Instant, HardwareSnapshot)> = None;

    while !shutdown::closing() {
        let demand = controller.demand();
        let now = Instant::now();
        if demand != previous {
            display_at = now;
            previous = demand;
        }
        if demand.active && now >= display_at {
            if main_visible(app) {
                let snapshot = sample_display(app, &hal, &controller, demand);
                latest = Some((Instant::now(), snapshot));
            }
            display_at = Instant::now() + DISPLAY_INTERVAL;
        }
        if Instant::now() >= tooltip_at {
            let fresh = latest
                .as_ref()
                .filter(|(at, _)| at.elapsed() <= DISPLAY_INTERVAL);
            tray::sample_tooltip(app, &hal, fresh.map(|(_, snapshot)| snapshot));
            tooltip_at = Instant::now() + TOOLTIP_INTERVAL;
        }
        let next = if demand.active {
            tooltip_at.min(display_at)
        } else {
            tooltip_at
        };
        controller.wait(demand, next);
    }
}

fn main_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main").is_some_and(|window| {
        window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(true)
    })
}

fn sample_display(
    app: &AppHandle,
    hal: &Hal,
    controller: &Controller,
    demand: Demand,
) -> HardwareSnapshot {
    let mut snapshot = hal.display_snapshot();
    let safety = hal.safety_status();
    if safety != crate::hal::SOFTWARE_FAN_IDLE_STATUS && safety != "监护中" {
        snapshot.warnings.push(safety);
    }
    if !shutdown::closing()
        && controller.demand() == demand
        && main_visible(app)
        && let Err(error) = app.emit_to("main", "hardware-snapshot", &snapshot)
    {
        log::error!("[monitoring] 发布硬件快照：{error:#}");
    }
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn new_demand_wakes_worker_without_waiting_for_tray_interval() {
        let controller = Controller::default();
        let idle = controller.demand();
        std::thread::scope(|scope| {
            let (ready, ready_rx) = mpsc::channel();
            let (done, done_rx) = mpsc::channel();
            let controller = &controller;
            scope.spawn(move || {
                ready.send(()).unwrap();
                controller.wait(idle, Instant::now() + TOOLTIP_INTERVAL);
                done.send(()).unwrap();
            });
            ready_rx.recv().unwrap();
            controller.set_active(true);
            done_rx.recv_timeout(DISPLAY_INTERVAL).unwrap();
        });
        let active = controller.demand();
        assert!(active.active);
        controller.set_active(true);
        assert_ne!(active.revision, controller.demand().revision);
        controller.set_active(false);
        assert!(!controller.demand().active);
    }
}
