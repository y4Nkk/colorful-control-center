use crate::{
    battery::calibration::{self, Access, Baseline, Event, Facts, power_plan},
    hal::power_plan::format_guid,
    storage,
};
use anyhow::{Result, bail};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(super) const OP: &str = "1234567890abcdef1234567890abcdef";
pub(super) const BEFORE: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub(super) fn load<T: DeserializeOwned>(root: &Path, name: &str) -> Result<T> {
    Ok(serde_json::from_slice(&fs::read(root.join(name))?)?)
}
pub(super) fn save(root: &Path, name: &str, data: &impl Serialize) -> Result<()> {
    storage::write(&root.join(name), &serde_json::to_vec(data)?)?;
    Ok(())
}
pub(super) fn baseline() -> Result<Baseline> {
    Ok(serde_json::from_value(
        json!({"stopThreshold":80,"powerScheme":BEFORE,"energy":null,
        "charge":{"current":{"enabled":true,"start":40,"stop":80},"startValues":[40,60],"stopValues":[80,100]}}),
    )?)
}
pub(super) fn initialize(root: &Path) -> Result<()> {
    save(
        root,
        "hardware.json",
        &json!({"ac":true,"refresh":false,"flexi":true}),
    )?;
    save(root, "charge.json", &baseline()?.charge.current)?;
    save(
        root,
        "facts.json",
        &json!({"present":true,"acOnline":true,"percent":60,"batteryStatus":0,"permanentFailure":0,"stopThreshold":80}),
    )?;
    save(
        root,
        "power.json",
        &json!({"active":BEFORE,"plans":{(BEFORE):"original"}}),
    )
}
pub(super) struct FileIo(pub PathBuf);
impl FileIo {
    fn power(&self) -> Result<(FilePower, String)> {
        Ok((
            FilePower(self.0.clone()),
            format_guid(power_plan::owned(OP)?),
        ))
    }
}
impl Access for FileIo {
    fn ready(&mut self) -> Result<()> {
        Ok(())
    }
    fn verify_baseline(&mut self, expected: &Baseline) -> Result<()> {
        if *expected != baseline()? {
            bail!("隔离基线不匹配");
        }
        Ok(())
    }
    fn facts(&mut self) -> Result<Facts> {
        let mut facts: Facts = load(&self.0, "facts.json")?;
        let hardware: Value = load(&self.0, "hardware.json")?;
        facts.ac_online = hardware["ac"].as_bool();
        Ok(facts)
    }
    fn events(&mut self) -> Result<Vec<Event>> {
        let path = self.0.join("events.json");
        if !path.exists() {
            return Ok(vec![]);
        }
        let codes: Vec<u8> = load(&self.0, "events.json")?;
        fs::remove_file(path)?;
        codes
            .into_iter()
            .map(|code| match code {
                85 => Ok(Event::FullStatus),
                90 => Ok(Event::AcUnplugged),
                91 => Ok(Event::FullyDischarged),
                _ => Err(anyhow::anyhow!("隔离事件码无效")),
            })
            .collect()
    }
    fn send(&mut self, command: i32, word: u32) -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.0.join("control.log"))?;
        writeln!(file, "{command}:{word:08x}")?;
        let mut state: Value = load(&self.0, "hardware.json")?;
        let field = match (command, word >> 24) {
            (121, 28) => "ac",
            (121, 29) => "refresh",
            (118, 7) => "flexi",
            _ => bail!("隔离传输拒绝未建模命令"),
        };
        state[field] = json!(word & 255 == 1);
        save(&self.0, "hardware.json", &state)
    }
    fn keep_awake(&mut self, enabled: bool) -> Result<()> {
        save(&self.0, "awake.json", &enabled)
    }
    fn restore_policy(&mut self, baseline: &Baseline) -> Result<()> {
        fs::write(self.0.join("restore.attempt"), b"attempt")?;
        if self.0.join("restore.blocked").exists() {
            bail!("隔离充电恢复暂不可用");
        }
        save(&self.0, "charge.json", &baseline.charge.current)
    }
    fn prepare_power_plan(&mut self, before: &str) -> Result<()> {
        let (io, target) = self.power()?;
        power_plan::prepare(&io, before, &target)
    }
    fn enter_power_plan(&mut self, before: &str) -> Result<()> {
        let (io, target) = self.power()?;
        power_plan::enter(&io, before, &target)
    }
    fn check_power_plan(&mut self) -> Result<()> {
        let (io, target) = self.power()?;
        power_plan::check(&io, &target)
    }
    fn restore_power_plan(&mut self, before: &str) -> Result<()> {
        let (io, target) = self.power()?;
        power_plan::restore(&io, before, &target)
    }
}
struct FilePower(PathBuf);
impl power_plan::Access for FilePower {
    fn active(&self) -> Result<String> {
        let data: Value = load(&self.0, "power.json")?;
        Ok(data["active"].as_str().unwrap().into())
    }
    fn exists(&self, id: &str) -> Result<bool> {
        let data: Value = load(&self.0, "power.json")?;
        Ok(data["plans"].get(id).is_some())
    }
    fn import(&self, id: &str) -> Result<()> {
        let mut data: Value = load(&self.0, "power.json")?;
        data["plans"][id] = json!(power_plan::bundle()?.sha256);
        fs::write(self.0.join("copy.pow"), power_plan::RESOURCE)?;
        save(&self.0, "power.json", &data)?;
        if self.0.join("import.partial").exists() {
            bail!("隔离导入部分失败");
        }
        Ok(())
    }
    fn verify(&self, id: &str) -> Result<()> {
        let data: Value = load(&self.0, "power.json")?;
        if data["plans"][id] != power_plan::bundle()?.sha256 {
            bail!("隔离电源回读不匹配");
        }
        Ok(())
    }
    fn activate(&self, id: &str) -> Result<()> {
        let mut data: Value = load(&self.0, "power.json")?;
        if data["plans"].get(id).is_none() {
            bail!("隔离目标计划不存在");
        }
        data["active"] = json!(id);
        save(&self.0, "power.json", &data)
    }
    fn delete(&self, id: &str) -> Result<()> {
        let mut data: Value = load(&self.0, "power.json")?;
        if data["active"] == id {
            bail!("隔离计划仍活动");
        }
        data["plans"].as_object_mut().unwrap().remove(id);
        save(&self.0, "power.json", &data)
    }
    fn cleanup(&self) -> Result<()> {
        match fs::remove_file(self.0.join("copy.pow")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
