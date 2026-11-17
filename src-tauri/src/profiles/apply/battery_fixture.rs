use crate::battery::{self, ChargeState, ChargeValues, Diagnostics, Io, PowerStatus, energy};
use anyhow::{Context, Result, bail};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct State {
    pub charge: Option<ChargeState>,
    pub present: Option<bool>,
    pub capability: Option<energy::Capability>,
    pub original: Option<energy::Policy>,
    pub commands: Vec<u32>,
    pub charges: Vec<ChargeValues>,
    pub events: Vec<String>,
    pub fail_commands: Vec<usize>,
    pub fail_charges: Vec<usize>,
}
#[derive(Default)]
pub struct Memory {
    pub state: Mutex<State>,
    pub after_command: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
pub struct Fixture {
    pub controller: battery::Controller,
    pub io: Arc<Memory>,
    pub dir: tempfile::TempDir,
}
impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let io = Arc::new(Memory::default());
        let controller = battery::Controller::new(dir.path().join("energy-policy.json"))
            .with_test_io(io.clone(), io.clone());
        Self {
            controller,
            io,
            dir,
        }
    }
    pub fn enable(&self) {
        let mut state = self.io.state.lock().unwrap();
        state.present = Some(true);
        state.charge = Some(ChargeState {
            current: ChargeValues {
                enabled: true,
                start: 60,
                stop: 80,
            },
            start_values: vec![40, 50, 60, 70, 80, 95],
            stop_values: vec![60, 70, 80, 90, 100],
        });
        state.capability = Some(
            serde_json::from_value(
                serde_json::json!({"factory":{"floor":95,"ceiling":100},"factoryDefaulted":false}),
            )
            .unwrap(),
        );
        state.original = Some(policy(false));
    }
    pub fn path(&self) -> std::path::PathBuf {
        self.dir.path().join("energy-policy.json")
    }
}
pub fn policy(enabled: bool) -> energy::Policy {
    serde_json::from_value(serde_json::json!({"enabled":enabled,"everyDay":false,"weekdays":62,
        "slots":[{"start":8,"end":18},{"start":20,"end":22}],"automatic":true,"limits":{"floor":95,"ceiling":100}})).unwrap()
}
impl Io for Memory {
    fn charge(&self) -> Result<Option<ChargeState>> {
        Ok(self.state.lock().unwrap().charge.clone())
    }
    fn power(&self) -> Result<PowerStatus> {
        Ok(PowerStatus {
            present: self.state.lock().unwrap().present,
            ..Default::default()
        })
    }
    fn diagnostics(&self) -> Result<Option<Diagnostics>> {
        Ok(None)
    }
    fn send(&self, values: &ChargeValues) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.charge.as_mut().context("隔离充电接口不可用")?.current = values.clone();
        state.charges.push(values.clone());
        state.events.push(format!(
            "CHARGE:{}:{}:{}",
            values.enabled, values.start, values.stop
        ));
        if state.fail_charges.contains(&state.charges.len()) {
            bail!("隔离充电部分写入后失败");
        }
        Ok(())
    }
}
impl energy::Io for Memory {
    fn capability(&self) -> Result<Option<energy::Capability>> {
        Ok(self.state.lock().unwrap().capability.clone())
    }
    fn charge(&self) -> Result<ChargeState> {
        Io::charge(self)?.context("隔离节能缺少充电恢复状态")
    }
    fn present(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().present == Some(true))
    }
    fn original(&self, _: &energy::Capability) -> Result<Option<energy::Policy>> {
        Ok(self.state.lock().unwrap().original.clone())
    }
    fn clock(&self) -> energy::Clock {
        energy::Clock {
            weekday: 2,
            hour: 12,
            minute: 30,
            second: 0,
        }
    }
    fn restore_charge(&self, charge: &ChargeState) -> Result<()> {
        energy::Io::charge(self)?.same_capabilities(charge)?;
        Io::send(self, &charge.current)?;
        if energy::Io::charge(self)? != *charge {
            bail!("隔离充电恢复回读不一致");
        }
        Ok(())
    }
    fn send(&self, command: u32) -> Result<()> {
        {
            let mut state = self.state.lock().unwrap();
            state.commands.push(command);
            state.events.push(format!("ENERGY:{command:08x}"));
            let charge = state.charge.as_mut().context("隔离充电接口不可用")?;
            if command >> 24 == 5 {
                charge.current.enabled = command & 1 != 0;
            }
            if command >> 24 == 6 {
                charge.current.start = command as u8;
                charge.current.stop = (command >> 8) as u8;
            }
            if state.fail_commands.contains(&state.commands.len()) {
                bail!("隔离节能部分输出后失败");
            }
        }
        let hook = self.after_command.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(())
    }
}
