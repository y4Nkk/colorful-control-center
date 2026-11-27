use super::super::{
    controller::Controller,
    model::{Record, Request, Selection},
    worker::{ApplicationIo, Worker},
};
use crate::{
    hal::power_events::{Event, Route, Source},
    powerctl::mode::profile::Grant,
    profiles::{
        self, ProfileFile, ProfileSettings,
        application::{self, Application, Origin, Permission, Runtime},
        apply::tests::fixtures::Fixture,
    },
};
use anyhow::Result;
use std::{cell::RefCell, path::PathBuf};

pub(in crate::profiles::policy) struct Harness {
    pub f: Fixture,
    pub dir: tempfile::TempDir,
    pub policy: Controller,
    pub application: application::Controller,
    worker: RefCell<Worker>,
}
impl Harness {
    pub fn context(&self) -> crate::powerctl::mode::links::Context<'_> {
        self.f
            .devices()
            .mode_context(&self.f.hal, &self.f.gpu)
            .unwrap()
    }
    pub fn new(display: bool) -> Self {
        let f = Fixture::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("profiles")).unwrap();
        let h = Self {
            f,
            policy: Controller::new(dir.path().join("power-profile-policy.json")),
            dir,
            application: application::Controller::default(),
            worker: RefCell::new(Worker::default()),
        };
        if display {
            h.f.display.enable();
        }
        for (name, watts) in [("插电", 75.0), ("电池", 55.0), ("唤醒", 65.0)] {
            let mut settings = ProfileSettings {
                pl1_w: Some(watts),
                ..Default::default()
            };
            if display && name == "电池" {
                let mut target = h.f.display.controller.capture().unwrap().unwrap();
                for display in &mut target.displays {
                    display.frequency = 144;
                }
                settings.display = Some(target);
            }
            h.write(&ProfileFile {
                version: 1,
                name: name.into(),
                saved_at_ms: 1,
                settings,
            });
        }
        h.policy
            .shared
            .ready(Route {
                modern_standby: Some(false),
                display_events: false,
                warnings: vec![],
            })
            .unwrap();
        h.observe(Event::Source(Source::Ac), 0);
        h
    }
    pub fn profiles(&self) -> PathBuf {
        self.dir.path().join("profiles")
    }
    pub fn file(&self, name: &str) -> ProfileFile {
        profiles::load_from(&self.profiles(), name).unwrap()
    }
    pub fn write(&self, file: &ProfileFile) {
        std::fs::write(
            self.profiles().join(format!("{}.json", file.name)),
            serde_json::to_vec(file).unwrap(),
        )
        .unwrap();
    }
    pub fn request(&self, resume: bool) -> Request {
        Request {
            expected: self.record(),
            enabled: true,
            ac: Some(Selection {
                file: self.file("插电"),
                mode: None,
            }),
            battery: Some(Selection {
                file: self.file("电池"),
                mode: None,
            }),
            resume: resume.then(|| Selection {
                file: self.file("唤醒"),
                mode: None,
            }),
        }
    }
    pub fn enable(&self, resume: bool) {
        self.policy
            .save(&self.context(), &self.profiles(), &self.request(resume))
            .unwrap();
    }
    pub fn record(&self) -> Record {
        self.policy.read().unwrap().record.unwrap()
    }
    pub fn observe(&self, event: Event, now: u64) {
        self.policy.shared.observe(event, now).unwrap();
    }
    pub fn tick(&self, now: u64) -> Result<()> {
        self.worker
            .borrow_mut()
            .tick(&self.policy.shared, &self.profiles(), now, self)
    }
    pub fn finish(&self) -> Result<()> {
        self.worker.borrow_mut().finish(self)
    }
    pub fn runtime(&self) -> Runtime<'_> {
        Runtime {
            origin: Origin::Manual,
            permission: None,
            hal: &self.f.hal,
            gpu: &self.f.gpu,
            devices: self.f.devices(),
            dir: self.dir.path(),
        }
    }
    pub fn last(&self) -> Option<Application> {
        self.application.read(&self.f.display.controller).unwrap()
    }
    pub fn watts(&self) -> i32 {
        i32::from_le_bytes(
            self.f.device.0.lock().unwrap().pages[6][33..37]
                .try_into()
                .unwrap(),
        )
    }
    pub fn begin_manual(&self) -> Application {
        let file = self.file("电池");
        let review = file.settings.display.as_ref().map(|target| {
            self.f
                .display
                .controller
                .preview_profile(target, None)
                .unwrap()
        });
        let dir = self.profiles();
        self.application
            .begin(
                Runtime {
                    dir: &dir,
                    ..self.runtime()
                },
                &file,
                None,
                review.as_ref(),
                None,
                None,
            )
            .unwrap()
    }
    pub fn keep(&self, id: u64) -> Result<Application> {
        let runtime = self.runtime();
        self.application.keep(runtime.hal, runtime.gpu, runtime.devices, id)
    }
}
impl ApplicationIo for Harness {
    fn begin(
        &self,
        file: &ProfileFile,
        mode: Option<&Grant>,
        origin: Origin,
        permission: Permission,
    ) -> Result<Application> {
        let (actual_mode, display) = {
            let _gate = self.f.hal.begin_write()?;
            (
                profiles::preview_authorization(&self.context(), &file.settings)?,
                file.settings
                    .display
                    .as_ref()
                    .map(|target| self.f.display.controller.preview_profile(target, None))
                    .transpose()?,
            )
        };
        let actual_mode = match (actual_mode, mode) {
            (Some(actual), Some(approved)) => {
                actual.check(approved)?;
                Some(actual.target)
            }
            (None, None) => None,
            _ => anyhow::bail!("自动档案模式联动已变化"),
        };
        let dir = self.profiles();
        self.application.begin(
            Runtime {
                origin,
                permission: Some(permission),
                dir: &dir,
                ..self.runtime()
            },
            file,
            actual_mode.as_ref(),
            display.as_ref(),
            None,
            None,
        )
    }
    fn current(&self) -> Result<Option<Application>> {
        self.application.read(&self.f.display.controller)
    }
    fn cancel(&self, id: u64) -> Result<Application> {
        let runtime = self.runtime();
        self.application.cancel(runtime.hal, runtime.devices.display, id)
    }
}
