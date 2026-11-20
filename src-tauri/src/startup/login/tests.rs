use super::{
    model::{self, Level, Record, State},
    transaction::{self, Io},
};
use anyhow::{Result, bail};
use std::cell::{Cell, RefCell};
const EXE: &str = r"C:\Apps\Colorful & Control\control.exe";
const USER: &str = "S-1-5-21-123-456-789-1001";
const NAME: &str = r"TESTPC\TestUser";
fn state(level: Level) -> State {
    State {
        task: model::xml(EXE, USER, level),
        run: None,
    }
}
struct Memory {
    state: RefCell<State>,
    record: RefCell<Option<Record>>,
    calls: Cell<usize>,
    fail: Vec<usize>,
}
impl Memory {
    fn new(before: State, fail: Vec<usize>) -> Self {
        Self {
            state: RefCell::new(before),
            record: RefCell::new(None),
            calls: Cell::new(0),
            fail,
        }
    }
    fn output(&self) -> Result<()> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if self.fail.contains(&call) {
            bail!("injected output failure {call}");
        }
        Ok(())
    }
}
impl Io for Memory {
    fn state(&self) -> Result<State> {
        Ok(self.state.borrow().clone())
    }
    fn record(&self) -> Result<Option<Record>> {
        Ok(self.record.borrow().clone())
    }
    fn task(&self, xml: Option<&str>) -> Result<()> {
        self.output()?;
        self.state.borrow_mut().task = xml.map(str::to_owned);
        Ok(())
    }
    fn run(&self, value: Option<&str>) -> Result<()> {
        self.output()?;
        self.state.borrow_mut().run = value.map(str::to_owned);
        Ok(())
    }
    fn save(&self, record: &Option<Record>) -> Result<()> {
        self.output()?;
        *self.record.borrow_mut() = record.clone();
        Ok(())
    }
}
#[test]
fn fixed_task_round_trips_all_permissions_and_escapes_path() {
    for level in [Level::Standard, Level::Administrator] {
        let task = model::xml(EXE, USER, level).unwrap();
        assert_eq!(model::task_level(&task, EXE, USER, NAME).unwrap(), level);
        assert!(task.contains("&amp;"));
        assert!(task.contains("InteractiveToken"));
        assert!(task.contains("--autostart"));
    }
    assert!(model::xml(EXE, USER, Level::Disabled).is_none());
}
// 2026-11-20 本机 schtasks 注册后任务服务实际存储形态：丢弃等于架构默认值的
// 元素（RunLevel=LeastPrivilege、Enabled、AllowStartOnDemand、Hidden、Priority、
// RunOnlyIfIdle、RunOnlyIfNetworkAvailable、WakeToRun、LogonTrigger/Enabled），
// LogonTrigger/UserId 的 SID 被规范化成 机器\用户，Principal/UserId 保留 SID，
// RegistrationInfo 补写日期。
fn normalized(run_level: Option<&str>) -> String {
    let principal = format!(
        "<UserId>{USER}</UserId><LogonType>InteractiveToken</LogonType>{}",
        run_level.map(|level| format!("<RunLevel>{level}</RunLevel>")).unwrap_or_default()
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
<RegistrationInfo><Date>2026-11-20T12:00:00</Date><Author>ColorfulControlCenter</Author><Description>ColorfulControlCenter login startup v1</Description><URI>\ColorfulControlCenter-{USER}</URI></RegistrationInfo>
<Triggers><LogonTrigger><UserId>{NAME}</UserId></LogonTrigger></Triggers>
<Principals><Principal id="Owner">{principal}</Principal></Principals>
<Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><AllowHardTerminate>false</AllowHardTerminate><StartWhenAvailable>true</StartWhenAvailable><IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings><ExecutionTimeLimit>PT0S</ExecutionTimeLimit></Settings>
<Actions Context="Owner"><Exec><Command>{exe}</Command><Arguments>--autostart</Arguments></Exec></Actions></Task>"#,
        exe = EXE.replace('&', "&amp;")
    )
}
#[test]
fn windows_normalized_task_matches_semantic_fingerprint() {
    assert_eq!(
        model::task_level(&normalized(None), EXE, USER, NAME).unwrap(),
        Level::Standard
    );
    assert_eq!(
        model::task_level(&normalized(Some("HighestAvailable")), EXE, USER, NAME).unwrap(),
        Level::Administrator
    );
    // 触发器与主体两种用户表示可各自独立出现
    let mixed = normalized(None).replace(NAME, USER);
    assert_eq!(
        model::task_level(&mixed, EXE, USER, NAME).unwrap(),
        Level::Standard
    );
}
#[test]
fn windows_normalized_task_tamper_is_rejected() {
    let task = normalized(Some("HighestAvailable"));
    for changed in [
        task.replace("--autostart", "--other"),
        task.replace(NAME, r"OTHERPC\Other"),
        task.replace(USER, "S-1-5-18"),
        task.replace("<AllowHardTerminate>false</AllowHardTerminate>", ""),
        task.replace("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>", ""),
        task.replace("<StartWhenAvailable>true</StartWhenAvailable>", ""),
        task.replace("<StartWhenAvailable>true", "<StartWhenAvailable>false"),
        task.replace(
            "<MultipleInstancesPolicy>IgnoreNew",
            "<MultipleInstancesPolicy>Parallel",
        ),
        task.replace("</Settings>", "<Volatile>true</Volatile></Settings>"),
        task.replace("</Triggers>", "<BootTrigger/></Triggers>"),
        task.replace(
            "</Actions>",
            "<Exec><Command>bad.exe</Command><Arguments>x</Arguments></Exec></Actions>",
        ),
        task.replace("/mit/task", "/other"),
        task.replace("Context=\"Owner\"", "Context=\"Other\""),
        task.replace("HighestAvailable", "SomethingElse"),
    ] {
        assert!(model::task_level(&changed, EXE, USER, NAME).is_err());
    }
}
#[test]
fn foreign_command_user_action_trigger_and_settings_are_rejected() {
    let task = model::xml(EXE, USER, Level::Administrator).unwrap();
    for changed in [
        task.replace("--autostart", "--other"),
        task.replace(USER, "S-1-5-18"),
        task.replace(
            "</Actions>",
            "<Exec><Command>bad.exe</Command></Exec></Actions>",
        ),
        task.replace("</Triggers>", "<BootTrigger/></Triggers>"),
        task.replace("<ExecutionTimeLimit>PT0S", "<ExecutionTimeLimit>PT1H"),
        task.replace("/mit/task", "/other"),
        task.replace("Context=\"Owner\"", "Context=\"Other\""),
    ] {
        assert!(model::task_level(&changed, EXE, USER, NAME).is_err());
    }
}
#[test]
fn legacy_item_migrates_once_and_first_restore_preserves_actual_old_value() {
    let original = State {
        task: None,
        run: Some(format!("\"{EXE}\" --autostart")),
    };
    let io = Memory::new(original.clone(), vec![]);
    let admin = state(Level::Administrator);
    transaction::apply(&io, &original, &None, &admin).unwrap();
    assert_eq!(io.state().unwrap(), admin);
    let record = io.record().unwrap();
    assert_eq!(record.as_ref().unwrap().before, original);
    let normal = state(Level::Standard);
    transaction::apply(&io, &admin, &record, &normal).unwrap();
    let record = io.record().unwrap();
    assert_eq!(record.as_ref().unwrap().before, original);
    transaction::apply(&io, &normal, &record, &original).unwrap();
    assert_eq!(io.state().unwrap(), original);
}
#[test]
fn stale_preview_and_record_have_zero_outputs() {
    let original = state(Level::Disabled);
    let io = Memory::new(state(Level::Standard), vec![]);
    assert!(transaction::apply(&io, &original, &None, &state(Level::Administrator)).is_err());
    assert_eq!(io.calls.get(), 0);
    let io = Memory::new(original.clone(), vec![]);
    let record = Some(Record {
        version: 1,
        before: original.clone(),
        applied: original.clone(),
    });
    assert!(transaction::apply(&io, &original, &record, &state(Level::Administrator)).is_err());
    assert_eq!(io.calls.get(), 0);
}
#[test]
fn every_write_failure_compensates_or_preserves_recovery_evidence() {
    let original = state(Level::Disabled);
    for call in 1..=3 {
        let io = Memory::new(original.clone(), vec![call]);
        assert!(transaction::apply(&io, &original, &None, &state(Level::Administrator)).is_err());
        assert_eq!(io.state().unwrap(), original);
        assert!(io.record().unwrap().is_none());
    }
    let io = Memory::new(original.clone(), vec![3, 4]);
    let error =
        transaction::apply(&io, &original, &None, &state(Level::Administrator)).unwrap_err();
    assert!(error.to_string().contains("恢复未完成"));
    assert_eq!(io.record().unwrap().unwrap().before, original);
    let current = io.state().unwrap();
    let record = io.record().unwrap();
    transaction::apply(&io, &current, &record, &original).unwrap();
    assert_eq!(io.state().unwrap(), original);
}
#[test]
fn binary_change_invalidates_preview() {
    let before = state(Level::Disabled);
    assert_ne!(
        model::revision(&before, &None, "a").unwrap(),
        model::revision(&before, &None, "b").unwrap()
    );
    assert!(
        model::validate(
            &State {
                task: None,
                run: Some("foreign.exe".into())
            },
            EXE,
            USER,
            NAME
        )
        .is_err()
    );
}

#[test]
fn legacy_run_is_removed_and_first_actual_state_survives_subsequent_changes() {
    let before = State {
        task: None,
        run: Some(format!("\"{EXE}\" --autostart")),
    };
    let io = Memory::new(before.clone(), vec![]);
    let standard = state(Level::Standard);
    transaction::apply(&io, &before, &None, &standard).unwrap();
    assert!(io.state().unwrap().run.is_none());
    let record = io.record().unwrap();
    transaction::apply(&io, &standard, &record, &state(Level::Administrator)).unwrap();
    let record = io.record().unwrap();
    assert_eq!(record.as_ref().unwrap().before, before);
    transaction::apply(&io, &state(Level::Administrator), &record, &before).unwrap();
    assert_eq!(io.state().unwrap(), before);
}
