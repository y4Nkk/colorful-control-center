use super::{
    Control,
    topology::{self, Part, Selection},
};
use crate::hal::audio::{Apartment, take_string};
use anyhow::{Context, Result, bail, ensure};
use std::{collections::BTreeMap, ptr};
use windows::{
    Win32::{
        Foundation::ERROR_NOT_FOUND,
        Media::Audio::{
            Connector, ConnectorType, DEVICE_STATE_ACTIVE, DEVICE_STATE_UNPLUGGED,
            IAudioAutoGainControl, IAudioMute, IAudioVolumeLevel, IConnector, IDeviceTopology,
            IMMDevice, IMMDeviceEnumerator, IPart, MMDeviceEnumerator, Subunit, eRender,
        },
        System::Com::{CLSCTX_ALL, CoCreateInstance},
    },
    core::{GUID, HRESULT, Interface},
};

fn incoming(part: &IPart) -> Result<Vec<IPart>> {
    let list = match unsafe { part.EnumPartsIncoming() } {
        Ok(list) => list,
        Err(error) if error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error.into()),
    };
    let count = unsafe { list.GetCount()? };
    ensure!(count <= 256, "音频上游节点数超出可核对范围");
    (0..count)
        .map(|index| Ok(unsafe { list.GetPart(index)? }))
        .collect()
}

fn describe(part: &IPart) -> Result<Part> {
    let mut node = Part::default();
    let kind = unsafe { part.GetPartType()? };
    if kind == Connector {
        let connector: IConnector = part.cast()?;
        node.terminal = matches!(
            unsafe { connector.GetType()? },
            ConnectorType::Software_IO | ConnectorType::Software_Fixed
        );
    } else {
        ensure!(kind == Subunit, "未知音频拓扑节点类型");
        let count = unsafe { part.GetControlInterfaceCount()? };
        ensure!(count <= 64, "音频控件数超出可核对范围");
        for index in 0..count {
            let iid = unsafe { part.GetControlInterface(index)?.GetIID()? };
            node.mute |= iid == IAudioMute::IID;
            node.volume |= iid == IAudioVolumeLevel::IID;
            node.gain |= iid == IAudioAutoGainControl::IID;
        }
        if node.volume {
            node.name = take_string(unsafe { part.GetName()? })?;
        }
    }
    node.sum =
        unsafe { part.GetSubType()? } == GUID::from_u128(0xda441a60_c556_11d0_8a2b_00a0c9255ac1);
    Ok(node)
}

fn topology(device: &IMMDevice) -> Result<(Option<Selection>, BTreeMap<String, IPart>)> {
    let topology: IDeviceTopology = unsafe { device.Activate(CLSCTX_ALL, None)? };
    let root: IPart = unsafe { topology.GetConnector(0)?.GetConnectedTo()? }.cast()?;
    let root_id = take_string(unsafe { root.GetGlobalId()? })?;
    let mut pending = vec![root];
    let mut nodes = BTreeMap::new();
    let mut handles = BTreeMap::new();
    while let Some(part) = pending.pop() {
        let id = take_string(unsafe { part.GetGlobalId()? })?;
        ensure!(!id.is_empty() && id.len() <= 4096, "音频节点身份无效");
        if nodes.contains_key(&id) {
            continue;
        }
        ensure!(nodes.len() < 256, "音频拓扑节点数超出可核对范围");
        let mut node = describe(&part)?;
        if !node.terminal {
            let upstream = incoming(&part)?;
            for child in &upstream {
                node.incoming
                    .push(take_string(unsafe { child.GetGlobalId()? })?);
            }
            pending.extend(upstream);
        }
        nodes.insert(id.clone(), node);
        handles.insert(id, part);
    }
    Ok((topology::select(&root_id, &nodes)?, handles))
}

fn activate(part: &IPart) -> Result<IAudioMute> {
    let mut raw = ptr::null_mut();
    unsafe {
        part.Activate(CLSCTX_ALL.0, &IAudioMute::IID, Some(&mut raw))?;
        ensure!(!raw.is_null(), "原厂PC Beep静音接口为空");
        Ok(IAudioMute::from_raw(raw))
    }
}

fn find() -> Result<(Control, IAudioMute)> {
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let devices = unsafe {
        enumerator.EnumAudioEndpoints(
            eRender,
            windows::Win32::Media::Audio::DEVICE_STATE(
                DEVICE_STATE_ACTIVE.0 | DEVICE_STATE_UNPLUGGED.0,
            ),
        )?
    };
    let count = unsafe { devices.GetCount()? };
    ensure!(count <= 128, "原厂播放端点数超出可核对范围");
    let mut errors = Vec::new();
    for index in 0..count {
        let device = unsafe { devices.Item(index)? };
        let (selection, handles) = match topology(&device) {
            Ok(parts) => parts,
            Err(error) => {
                log::debug!("[pc-beep] 端点{index}未提供可核对拓扑：{error:#}");
                errors.push(format!("端点{index}：{error:#}"));
                continue;
            }
        };
        let Some(selection) = selection else {
            continue;
        };
        let mute_part = selection
            .mute
            .context("首个PC Beep控制组没有可读取的静音控件")?;
        let volume_part = selection
            .volume
            .context("原厂PC Beep控制组缺少音量节点身份")?;
        let mute = activate(handles.get(&mute_part).context("原厂静音节点已变化")?)?;
        let current = Control {
            endpoint: take_string(unsafe { device.GetId()? })?,
            volume_part,
            mute_part,
            muted: unsafe { mute.GetMute()?.as_bool() },
        };
        current.validate()?;
        return Ok((current, mute));
    }
    bail!(
        "未发现可核对的原厂PC Beep控制组{}",
        if errors.is_empty() {
            String::new()
        } else {
            format!("：{}", errors.join("；"))
        }
    )
}

pub(crate) fn capture() -> Result<Control> {
    let _apartment = Apartment::enter()?;
    find().map(|(current, _)| current)
}

pub(crate) fn set_muted(expected: &Control, muted: bool) -> Result<Control> {
    let _apartment = Apartment::enter()?;
    let (before, control) = find()?;
    ensure!(&before == expected, "原厂PC Beep控件或静音状态已变化");
    unsafe {
        control.SetMute(muted, None)?;
    }
    ensure!(
        unsafe { control.GetMute()?.as_bool() } == muted,
        "原厂PC Beep静音回读不一致"
    );
    let (after, _) = find()?;
    ensure!(
        after.same_device(expected) && after.muted == muted,
        "原厂PC Beep写后身份或状态已变化"
    );
    Ok(after)
}

pub(crate) use super::wave::silence;
