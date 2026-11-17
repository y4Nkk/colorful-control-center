//! LAN显式内存环境；测试不回退真实设备、注册表或系统授权。
use super::{
    authorization::{Authorized, Task},
    model::*,
    worker::{Execution, Operation},
    *,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
pub(crate) type Hook = Box<dyn FnOnce() + Send>;
pub(crate) struct Memory {
    pub frame: Mutex<Frame>,
    pub gate: Mutex<Option<Arc<Mutex<()>>>>,
    pub on_authorize: Mutex<Option<Hook>>,
    pub before_execute: Mutex<Option<Hook>>,
    pub after_write: Mutex<Option<Hook>>,
    pub authorizations: AtomicUsize,
    pub executions: AtomicUsize,
    pub cancellations: AtomicUsize,
    pub rollbacks: AtomicUsize,
    pub finishes: AtomicUsize,
    pub writes: AtomicUsize,
    pub failures: Mutex<Vec<usize>>,
    pub refused: Mutex<Vec<usize>>,
    pub owner: AtomicBool,
    pub deny: AtomicBool,
    pub fail_finish: AtomicBool,
    path: PathBuf,
}
#[derive(Clone)]
struct Io(Arc<Memory>);
impl transaction::Io for Io {
    fn inspect(&self, instance: &str) -> Result<Device> {
        Ok(self.0.frame.lock().unwrap().device(instance)?.clone())
    }
    fn prepare(&self, _: &Device, _: &Change) -> Result<()> {
        bail!("必须先持久记录");
    }
    fn write(&self, identity: &Identity, field: Field, value: &Option<Value>) -> Result<()> {
        let record = store::read(&self.0.path)?;
        if !record
            .devices
            .iter()
            .any(|device| device.identity == *identity && device.fields.contains_key(&field))
        {
            bail!("隔离写入之前尚未持久记录字段");
        }
        let number = self.0.writes.fetch_add(1, Ordering::SeqCst) + 1;
        if self.0.refused.lock().unwrap().contains(&number) {
            bail!("隔离写入被拒绝");
        }
        {
            let mut frame = self.0.frame.lock().unwrap();
            let device = frame
                .devices
                .iter_mut()
                .find(|device| device.identity == *identity)
                .context("隔离设备身份变化")?;
            device.values.insert(field, value.clone());
        }
        let hook = self.0.after_write.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        if self.0.failures.lock().unwrap().contains(&number) {
            bail!("隔离写入后故障");
        }
        Ok(())
    }
}
struct Env(Arc<Memory>);
impl Environment for Env {
    fn frame(&self, hal: &Hal, _: bool) -> Result<Frame> {
        self.0
            .gate
            .lock()
            .unwrap()
            .get_or_insert_with(|| hal.control_gate());
        Ok(self.0.frame.lock().unwrap().clone())
    }
    fn ensure_owner_absent(&self) -> Result<()> {
        if self.0.owner.load(Ordering::SeqCst) {
            bail!("隔离原厂控制者出现");
        }
        Ok(())
    }
    fn authorize(&self, operation: Operation) -> Result<Box<dyn Authorized>> {
        let gate = self
            .0
            .gate
            .lock()
            .unwrap()
            .clone()
            .context("未注入HAL来源")?;
        assert!(gate.try_lock().is_ok(), "系统授权等待不能持有HAL");
        self.0.authorizations.fetch_add(1, Ordering::SeqCst);
        let hook = self.0.on_authorize.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        if self.0.deny.load(Ordering::SeqCst) {
            bail!("隔离用户取消授权");
        }
        Ok(Box::new(Permit {
            memory: self.0.clone(),
            operation,
            task: None,
            finished: false,
        }))
    }
}
struct Permit {
    memory: Arc<Memory>,
    operation: Operation,
    task: Option<Execution<Io>>,
    finished: bool,
}
impl Authorized for Permit {
    fn execute(&mut self) -> Result<()> {
        let gate = self.memory.gate.lock().unwrap().clone().unwrap();
        assert!(gate.try_lock().is_err(), "实际执行必须持有HAL");
        self.memory.executions.fetch_add(1, Ordering::SeqCst);
        if self.task.is_some() {
            bail!("隔离会话不能重复执行");
        }
        let hook = self.memory.before_execute.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        let frame = self.memory.frame.lock().unwrap().clone();
        let record = store::read(&self.operation.recovery_file)?;
        self.task = Some(Execution::prepare(
            &self.operation,
            &frame,
            record,
            Io(self.memory.clone()),
        )?);
        self.task.as_mut().unwrap().execute()
    }
    fn rollback(&mut self) -> Result<()> {
        let gate = self.memory.gate.lock().unwrap().clone().unwrap();
        assert!(gate.try_lock().is_err(), "本次补偿必须持有HAL");
        self.memory.rollbacks.fetch_add(1, Ordering::SeqCst);
        self.task.as_mut().map(Task::rollback).unwrap_or(Ok(()))
    }
    fn dirty(&self) -> bool {
        self.task.as_ref().is_some_and(Task::dirty)
    }
    fn finish(mut self: Box<Self>) -> Result<()> {
        self.finished = true;
        self.memory.finishes.fetch_add(1, Ordering::SeqCst);
        if self.task.is_none() {
            self.memory.cancellations.fetch_add(1, Ordering::SeqCst);
        }
        if self.memory.fail_finish.load(Ordering::SeqCst) {
            bail!("隔离授权收尾失败");
        }
        Ok(())
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        if !self.finished {
            self.memory.cancellations.fetch_add(1, Ordering::SeqCst);
            if let Some(task) = self.task.as_mut().filter(|task| task.dirty()) {
                let _ = task.rollback();
            }
        }
    }
}
pub(crate) struct Fixture {
    pub controller: Controller,
    pub io: Arc<Memory>,
    pub directory: tempfile::TempDir,
}
impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lan-recovery.json");
        let io = Arc::new(Memory {
            frame: Mutex::new(Frame {
                capabilities: Capabilities::default(),
                recommendation: None,
                devices: Vec::new(),
            }),
            gate: Mutex::new(None),
            on_authorize: Mutex::new(None),
            before_execute: Mutex::new(None),
            after_write: Mutex::new(None),
            authorizations: AtomicUsize::new(0),
            executions: AtomicUsize::new(0),
            cancellations: AtomicUsize::new(0),
            rollbacks: AtomicUsize::new(0),
            finishes: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
            failures: Mutex::new(Vec::new()),
            refused: Mutex::new(Vec::new()),
            owner: AtomicBool::new(false),
            deny: AtomicBool::new(false),
            fail_finish: AtomicBool::new(false),
            path: path.clone(),
        });
        let controller = Controller::with_test(path, Arc::new(Env(io.clone())));
        Self {
            controller,
            io,
            directory,
        }
    }
    pub fn path(&self) -> PathBuf {
        self.directory.path().join("lan-recovery.json")
    }
    pub fn enable(&self) {
        *self.io.frame.lock().unwrap() = Frame {
            capabilities: Capabilities {
                power_saving: true,
                wake_patch: true,
            },
            recommendation: Some(true),
            devices: vec![device()],
        };
    }
}
pub(crate) fn device() -> Device {
    let mut values = FIELDS
        .into_iter()
        .map(|field| (field, None))
        .collect::<std::collections::BTreeMap<_, _>>();
    values.insert(Field::PowerSavingMode, Some(Value::string("0")));
    values.insert(Field::WakeOnSlot, Some(Value::string("1")));
    Device {
        identity: Identity {
            instance: "PCI\\VEN_ABCD&DEV_1234\\1".into(),
            driver: format!("{CLASS}\\0001"),
            net_cfg_id: Some(Value::string("isolated-id")),
            driver_version: Some(Value::string("1.0")),
            matching_id: None,
        },
        name: "隔离网卡".into(),
        values,
    }
}
