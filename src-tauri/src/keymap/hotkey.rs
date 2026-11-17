use super::*;

fn content(frame: &Frame) -> Result<Configuration> {
    if !supported(&frame.facts.capability)? {
        bail!("固件未声明FlexiKey能力");
    }
    let record = frame
        .loaded
        .record
        .as_ref()
        .context("请先应用并接管需要切换的映射设备")?;
    if record.phase != model::Phase::Applied {
        bail!("上次映射需要恢复");
    }
    let mut target = model::subset(&record.current, &record.owned)?;
    for device in &mut target.devices {
        device.enabled = false;
    }
    Ok(target)
}
fn identity(frame: &Frame) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&content(frame)?)?)
    ))
}
impl Controller {
    pub(crate) fn recording_active(&self) -> Result<bool> {
        self.recording.active()
    }
    pub(crate) fn hotkey_identity(&self, hal: &Hal) -> Result<String> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.with_io(hal, |io| {
            identity(&capture(io, &store::FileStore(self.path.clone()))?)
        })
    }
    pub(crate) fn toggle_owned(&self, hal: &Hal, expected: &str) -> Result<bool> {
        let mut scope = self.scope()?;
        if self.recording.active()? {
            bail!("宏录制中，未切换映射");
        }
        let (plan, enabled) = {
            let _gate = hal.begin_write()?;
            scope.with_io(hal, |io| {
                let frame = capture(io, &store::FileStore(self.path.clone()))?;
                frame.review(&frame.revision()?, false)?;
                if identity(&frame)? != expected {
                    bail!("已接管映射内容变化，请重新确认热键范围");
                }
                let mut target = content(&frame)?;
                let enabled = !target
                    .devices
                    .iter()
                    .any(|d| frame.facts.flags[model::index(d.device)] == 1);
                for device in &mut target.devices {
                    device.enabled = enabled;
                }
                Ok((
                    transaction::prepare(
                        frame,
                        &model::Target {
                            configuration: target,
                            programs_enabled: true,
                        },
                    )?,
                    enabled,
                ))
            })?
        };
        scope.run_single(hal, &plan)?;
        Ok(enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hotkey_content_only_contains_owned_devices_and_ignores_enabled_flips() {
        let configuration = Configuration {
            devices: vec![
                keymap::DeviceSettings {
                    device: Device::Keyboard,
                    enabled: true,
                    bindings: vec![],
                },
                keymap::DeviceSettings {
                    device: Device::Mouse,
                    enabled: true,
                    bindings: vec![],
                },
            ],
        };
        let mut frame = Frame {
            facts: Facts {
                capability: {
                    let mut c = [0; 15];
                    c[0] = 1;
                    c[13] = 2;
                    c
                },
                flags: [1, 1],
                original: None,
                original_error: None,
            },
            loaded: store::Loaded {
                record: Some(model::Record {
                    version: 1,
                    baseline: model::Baseline {
                        name: "初始".into(),
                        configuration: configuration.clone(),
                        flags: [1, 1],
                    },
                    current: configuration,
                    owned: vec![Device::Keyboard],
                    phase: model::Phase::Applied,
                }),
                revision: "fake".into(),
            },
            blocked: None,
        };
        let digest = identity(&frame).unwrap();
        assert_eq!(content(&frame).unwrap().devices.len(), 1);
        frame.loaded.record.as_mut().unwrap().current.devices[0].enabled = false;
        assert_eq!(digest, identity(&frame).unwrap());
        frame
            .loaded
            .record
            .as_mut()
            .unwrap()
            .owned
            .push(Device::Mouse);
        assert_ne!(digest, identity(&frame).unwrap());
        frame.loaded.record.as_mut().unwrap().phase = model::Phase::Prepared;
        assert!(identity(&frame).is_err());
    }
}
