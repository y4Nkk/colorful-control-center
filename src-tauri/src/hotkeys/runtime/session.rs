use super::*;

pub(in crate::hotkeys) struct Session {
    activate: Option<mpsc::Sender<()>>,
    started: mpsc::Receiver<()>,
    stop: mpsc::Sender<()>,
    worker: Option<thread::JoinHandle<Result<()>>>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.activate.take();
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            match worker.join() {
                Ok(Err(error)) => log::error!("[hotkeys] 会话清理：{error:#}"),
                Err(_) => log::error!("[hotkeys] 会话清理线程异常"),
                _ => (),
            }
        }
    }
}
impl Session {
    pub fn prepare(
        app: AppHandle,
        path: PathBuf,
        record: Record,
        status: Arc<Mutex<Status>>,
    ) -> Result<Self> {
        let (ready, waiting) = mpsc::sync_channel(1);
        let (activate, activation) = mpsc::channel();
        let (started, started_wait) = mpsc::sync_channel(1);
        let (stop, stopping) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("oem-hotkeys".into())
            .spawn(move || {
                let alert_scope = record.settings.actions.contains(&Scope::BatteryAlert);
                let (result, cleanup) = Worker {
                    app: &app,
                    path: &path,
                    record,
                    status: &status,
                    ready,
                    activation,
                    started,
                    stopping,
                    cleanup: Ok(()),
                }
                .run();
                if let Ok(mut state) = status.lock() {
                    state.running = false;
                    if result.is_ok() {
                        state.stop_clean();
                    }
                    if let Err(error) = &result {
                        state.error = Some(format!("{error:#}"));
                        log::error!("[hotkeys] {error:#}");
                    }
                }
                publish(&app, &status);
                if alert_scope {
                    if let Err(error) = app.emit_to("main", "hardware-controls-changed", ()) {
                        log::warn!("[hotkeys] 电池音频结束刷新：{error}");
                    }
                }
                cleanup
            })?;
        let session = Self {
            activate: Some(activate),
            started: started_wait,
            stop,
            worker: Some(worker),
        };
        waiting
            .recv_timeout(Duration::from_secs(5))
            .context("准备热键监听超时")?
            .map_err(anyhow::Error::msg)?;
        Ok(session)
    }
    pub fn activate(&mut self) -> Result<()> {
        self.activate
            .take()
            .context("热键监听已经启动")?
            .send(())
            .map_err(|_| anyhow!("热键监听启动失败"))?;
        self.started
            .recv_timeout(Duration::from_secs(3))
            .context("确认热键监听启动超时")?;
        Ok(())
    }
    pub fn finished(&self) -> bool {
        self.worker
            .as_ref()
            .is_none_or(thread::JoinHandle::is_finished)
    }
    pub fn stop(mut self) -> Result<()> {
        self.activate.take();
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow!("热键监听线程异常，清理结果未知"))??;
        }
        Ok(())
    }
}
struct Worker<'a> {
    app: &'a AppHandle,
    path: &'a std::path::Path,
    record: Record,
    status: &'a Arc<Mutex<Status>>,
    ready: mpsc::SyncSender<std::result::Result<(), String>>,
    activation: mpsc::Receiver<()>,
    started: mpsc::SyncSender<()>,
    stopping: mpsc::Receiver<()>,
    cleanup: Result<()>,
}
impl Worker<'_> {
    fn run(mut self) -> (Result<()>, Result<()>) {
        let result = self.run_inner();
        (result, self.cleanup)
    }
    fn run_inner(&mut self) -> Result<()> {
        let (mut log, mut events) = match battery_events::prepare(&self.record) {
            Ok(sources) => sources,
            Err(error) => {
                let _ = self.ready.send(Err(format!("{error:#}")));
                return Err(error);
            }
        };
        self.ready
            .send(Ok(()))
            .map_err(|_| anyhow!("热键准备已取消"))?;
        if self.activation.recv().is_err() {
            return Ok(());
        }
        let alert = self.record.settings.actions.contains(&Scope::BatteryAlert);
        let controller = self.app.state::<super::super::Controller>();
        if alert {
            controller.battery_alert.start()?;
        }
        let consumed = (|| {
            if let Ok(mut state) = self.status.lock() {
                state.running = true;
                state.error = None;
            }
            self.started
                .send(())
                .map_err(|_| anyhow!("热键启动已取消"))?;
            publish(self.app, self.status);
            consume(
                self.app,
                self.path,
                &mut self.record,
                self.status,
                &self.stopping,
                &mut log,
                &mut events,
            )
        })();
        let panel = controller.panel_hotkey.end(None);
        let audio = if alert {
            controller.battery_alert.cleanup(
                &self.app.state::<crate::hal::Hal>(),
                &controller.screen_brightness,
            )
        } else {
            Ok(())
        };
        self.cleanup = match (panel, audio) {
            (Err(first), Err(second)) => Err(anyhow!("{first:#}；{second:#}")),
            (Err(error), _) | (_, Err(error)) => Err(error),
            _ => Ok(()),
        };
        match (consumed, &self.cleanup) {
            (Err(error), Err(cleanup)) => Err(anyhow!("{error:#}；{cleanup:#}")),
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(anyhow!("{error:#}")),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stop_waits_for_owned_worker_and_propagates_cleanup_error() {
        let (stop, stopping) = mpsc::channel();
        let (_, started) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            stopping.recv().unwrap();
            Err(anyhow!("隔离音频恢复失败"))
        });
        let session = Session {
            activate: None,
            started,
            stop,
            worker: Some(worker),
        };
        assert!(
            session
                .stop()
                .unwrap_err()
                .to_string()
                .contains("音频恢复失败")
        );
    }
    #[test]
    fn dropping_an_unactivated_session_closes_activation_and_stops_the_worker() {
        let (activate, activation) = mpsc::channel();
        let (stop, stopping) = mpsc::channel();
        let (_, started) = mpsc::sync_channel(1);
        let (completed, received) = mpsc::channel();
        let worker = thread::spawn(move || {
            assert!(activation.recv().is_err());
            stopping.recv().unwrap();
            completed.send(()).unwrap();
            Ok(())
        });
        drop(Session {
            activate: Some(activate),
            started,
            stop,
            worker: Some(worker),
        });
        received.recv_timeout(Duration::from_secs(1)).unwrap();
    }
}
