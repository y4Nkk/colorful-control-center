//! 复用生产任务、客户端、恢复记录及电源事务；全部硬件出口只读写私有目录。
#[path = "calibration_fixture/io.rs"]
mod io;
use crate::{
    battery::calibration::{
        self, Phase, Status,
        client::Client,
        store::{self, FileStore, Record, Store},
        worker,
    },
    controller_lease::ControllerLease,
};
use anyhow::{Context, Result, bail};
use io::{BEFORE, FileIo, OP, baseline, initialize, load, save};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

fn child(root: &Path, lease: &str, scenario: &str) -> Result<()> {
    let lease = ControllerLease::retain(lease)?;
    fs::write(root.join("child.pid"), std::process::id().to_string())?;
    if scenario == "never-ready" {
        std::thread::sleep(Duration::from_secs(5));
    }
    let store = FileStore(root.join(calibration::RECORD_NAME));
    let input = worker::read(std::io::stdin());
    worker::run(input, &mut FileIo(root.into()), &store, |message| {
        if matches!(&message, worker::Output::State { state } if state.active()) {
            fs::write(root.join("worker.active"), b"active")?;
            if scenario == "child-exit" {
                std::process::exit(33);
            }
        }
        let mut output = std::io::stdout().lock();
        writeln!(output, "{}", serde_json::to_string(&message)?)?;
        output.flush()?;
        Ok(())
    })?;
    drop(lease);
    fs::write(root.join("child.exit"), b"finished")?;
    Ok(())
}
fn drive_completion(root: &Path, phase: Phase, previous: &mut Option<Phase>) -> Result<()> {
    if previous.as_ref() == Some(&phase) {
        return Ok(());
    }
    *previous = Some(phase);
    let mut facts: Value = load(root, "facts.json")?;
    let event = match phase {
        Phase::Discharge1 | Phase::Discharge2 => {
            facts["percent"] = json!(10);
            91
        }
        Phase::Charge1 | Phase::Charge2 => {
            facts["percent"] = json!(100);
            facts["batteryStatus"] = json!(32);
            85
        }
        Phase::Adjusting => {
            facts["percent"] = json!(80);
            0
        }
    };
    save(root, "facts.json", &facts)?;
    if event != 0 {
        save(root, "events.json", &vec![event])?;
    }
    Ok(())
}
fn run_parent(root: &Path, scenario: &str, client: &mut Client, store: &FileStore) -> Result<()> {
    let mut stopped = false;
    let mut previous = None;
    let mut silent = false;
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        let record = store.load()?.context("隔离记录缺失")?;
        if scenario == "heartbeat-loss" && record.state.as_ref().is_some_and(|s| s.active()) {
            silent = true;
        }
        if !silent && !client.pulse()? {
            if let Some(mut record) = store.load()?.filter(|r| r.pending) {
                record.set_error("隔离工作进程提前退出".into());
                store::recover(&mut FileIo(root.into()), store, record)?;
            }
            return Ok(());
        }
        if silent && !record.pending {
            return Ok(());
        }
        if let Some(state) = &record.state {
            if scenario == "stop" && state.active() && !stopped {
                client.stop()?;
                stopped = true;
            }
            if scenario == "complete" && state.active() {
                drive_completion(root, state.phase, &mut previous)?;
            }
            if scenario == "plan-change" && state.active() && !stopped {
                let mut power: Value = load(root, "power.json")?;
                power["active"] = json!(BEFORE);
                save(root, "power.json", &power)?;
                stopped = true;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("隔离父进程等待超时");
}
fn parent(root: &Path, lease: &str, scenario: &str) -> Result<()> {
    let lease = ControllerLease::try_acquire(lease)?.context("隔离控制权已占用")?;
    initialize(root)?;
    let store = FileStore(root.join(calibration::RECORD_NAME));
    store.save(&Record::prepared(OP.into(), baseline()?)?)?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("child")
        .arg(root)
        .arg(lease_name(root)?)
        .arg(scenario);
    let started = Client::spawn(command, OP.into());
    if scenario == "never-ready" {
        assert!(started.is_err());
        store::recover(&mut FileIo(root.into()), &store, store.load()?.unwrap())?;
        fs::write(root.join("start.rejected"), b"rejected")?;
    } else {
        let mut client = started?;
        client.arm()?;
        fs::write(root.join("parent.ready"), b"ready")?;
        run_parent(root, scenario, &mut client, &store)?;
    }
    drop(lease);
    fs::write(root.join("parent.exit"), b"finished")?;
    Ok(())
}
fn lease_name(root: &Path) -> Result<String> {
    Ok(fs::read_to_string(root.join("lease.name"))?)
}
pub fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        bail!("隔离任务参数不足");
    }
    let root = Path::new(&args[1]).canonicalize()?;
    if fs::read(root.join("fixture.marker"))? != b"ccc-calibration-fixture" {
        bail!("隔离目录标记无效");
    }
    if args[2] != lease_name(&root)? {
        bail!("隔离控制权不匹配");
    }
    match args[0].as_str() {
        "parent" if args.len() == 4 => parent(&root, &args[2], &args[3]),
        "child" if args.len() == 5 && args[4] == "--battery-calibration" => {
            child(&root, &args[2], &args[3])
        }
        _ => bail!("隔离任务参数无效"),
    }
}
