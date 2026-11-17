//! 设备、授权与HAL均显式隔离，缺注入时禁止回退原生提供者。
use super::{
    environment::Environment,
    execution::Execution,
    operation::Io,
    request::{self, Invocation, Request},
    *,
};
use crate::hal::{Hal, authorization::session::Authorized};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
pub(crate) struct Memory(pub Arc<Mutex<Data>>);
pub(crate) struct Data {
    pub supported: bool,
    pub temperature: Option<f64>,
    pub node_hook: Option<Arc<dyn Fn(bool) + Send + Sync>>,
    pub device: Device,
    pub firmware: bool,
    pub events: Vec<String>,
    pub failures: Vec<usize>,
    pub authorize_hook: Option<Box<dyn FnOnce() + Send>>,
    pub deny: bool,
    pub fail_finish: bool,
}
impl Default for Memory {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Data {
            supported: true,
            temperature: Some(54.0),
            node_hook: None,
            device: Device {
                id: "ACPI\\NVDA0820\\ISOLATED".into(),
                name: "Memory".into(),
                enabled: true,
                automatic_power_off: false,
                problem: 0,
                drives_display: false,
                can_disable: true,
            },
            firmware: true,
            events: vec![],
            failures: vec![],
            authorize_hook: None,
            deny: false,
            fail_finish: false,
        })))
    }
}
impl Memory {
    pub fn device(&self) -> Device {
        self.0.lock().unwrap().device.clone()
    }
    pub fn request(&self, enabled: bool) -> Request {
        Request {
            expected: self.device(),
            target: Target::requested(enabled),
            restore: false,
        }
    }
    fn change(&self, name: String, mutate: impl FnOnce(&mut Data)) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.events.push(name);
        mutate(&mut data);
        if data.failures.contains(&data.events.len()) {
            bail!("隔离独显写入失败");
        }
        Ok(())
    }
}
impl Io for Memory {
    fn inspect(&self, id: &str) -> Result<Device> {
        if !self.0.lock().unwrap().supported {
            bail!("隔离独显能力不存在");
        }
        let device = self.device();
        if device.id != id {
            bail!("隔离独显实例已变化");
        }
        Ok(device)
    }
    fn firmware(&self, enabled: bool) -> Result<()> {
        self.change(format!("firmware:{enabled}"), |d| d.firmware = enabled)
    }
    fn node(&self, _: &str, enabled: bool) -> Result<()> {
        self.change(format!("node:{enabled}"), |d| {
            d.device.enabled = enabled;
            d.device.problem = if enabled { 0 } else { 22 };
        })?;
        let hook = self.0.lock().unwrap().node_hook.clone();
        if let Some(hook) = hook {
            hook(enabled);
        }
        Ok(())
    }
    fn save(&self, automatic: bool) -> Result<()> {
        self.change(format!("automatic:{automatic}"), |d| {
            d.device.automatic_power_off = automatic
        })
    }
}
pub(crate) struct EnvironmentFixture {
    pub io: Memory,
    pub authorizations: AtomicUsize,
    pub prepared: Arc<AtomicUsize>,
}
impl EnvironmentFixture {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            io: Memory::default(),
            authorizations: AtomicUsize::new(0),
            prepared: Arc::new(AtomicUsize::new(0)),
        })
    }
}
impl Environment for EnvironmentFixture {
    fn supported(&self, _: &Hal) -> Result<bool> {
        Ok(self.io.0.lock().unwrap().supported)
    }
    fn list(&self, _: &Hal) -> Result<Vec<Device>> {
        let supported = self.io.0.lock().unwrap().supported;
        Ok(if supported {
            vec![self.io.device()]
        } else {
            vec![]
        })
    }
    fn authorize(&self, sequence: super::sequence::Sequence) -> Result<Box<dyn Authorized>> {
        self.authorizations.fetch_add(1, Ordering::SeqCst);
        let hook = self.io.0.lock().unwrap().authorize_hook.take();
        if let Some(hook) = hook {
            hook();
        }
        let denied = self.io.0.lock().unwrap().deny;
        if denied {
            bail!("隔离系统授权取消");
        }
        let io = self.io.clone();
        let result_source = io.clone();
        let prepared = self.prepared.clone();
        request::prepare_with(sequence, move |arguments| {
            let invocation = Invocation::from_args(
                &arguments.split(' ').map(str::to_owned).collect::<Vec<_>>(),
            )?;
            request::serve(&invocation, move |request| {
                prepared.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(Execution::new(io.clone(), request)))
            })?;
            if result_source.0.lock().unwrap().fail_finish {
                bail!("隔离独显授权收尾未确认");
            }
            Ok(0)
        })
    }
}
pub(crate) fn hal() -> Hal {
    unsafe extern "system" fn buffer(_: i32, _: *mut u8) -> i32 {
        0
    }
    unsafe extern "system" fn read(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
        0
    }
    unsafe extern "system" fn set(_: i32, _: *const u8, _: i32) -> i32 {
        0
    }
    unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
        0
    }
    Hal::test_instance(
        crate::hal::oem_dchu::Dchu::test_io(buffer, read, set, write),
        None,
    )
}
