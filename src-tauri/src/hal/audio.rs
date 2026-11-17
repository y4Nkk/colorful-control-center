use crate::microphone::Endpoint;
use anyhow::{Result, bail};
use windows::{
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::RPC_E_CHANGED_MODE,
        Media::Audio::{
            DEVICE_STATE_ACTIVE, Endpoints::IAudioEndpointVolume, IMMDevice, IMMDeviceEnumerator,
            IMMEndpoint, MMDeviceEnumerator, eCapture,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
            CoUninitialize, STGM_READ,
            StructuredStorage::{PropVariantClear, PropVariantToStringAlloc},
        },
    },
    core::{Interface, PCWSTR, PWSTR},
};

pub(super) struct Apartment(bool);
impl Apartment {
    pub(super) fn enter() -> Result<Self> {
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result == RPC_E_CHANGED_MODE {
            return Ok(Self(false));
        }
        result.ok()?;
        Ok(Self(true))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}
pub(super) fn take_string(value: PWSTR) -> Result<String> {
    if value.is_null() {
        bail!("音频设备返回空字符串");
    }
    let text = unsafe { value.to_string() };
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    Ok(text?)
}
fn friendly_name(device: &IMMDevice) -> Result<String> {
    let mut value = unsafe {
        device
            .OpenPropertyStore(STGM_READ)?
            .GetValue(&PKEY_Device_FriendlyName)?
    };
    let text = unsafe { PropVariantToStringAlloc(&value) };
    let clear = unsafe { PropVariantClear(&mut value) };
    let result = text.map_err(anyhow::Error::from).and_then(take_string);
    clear?;
    result
}
fn enumerator() -> Result<IMMDeviceEnumerator> {
    Ok(unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? })
}
pub(crate) fn capture() -> Result<Vec<Endpoint>> {
    let _apartment = Apartment::enter()?;
    let enumerator = enumerator()?;
    let collection = unsafe { enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)? };
    let count = unsafe { collection.GetCount()? };
    if count > 128 {
        bail!("活动麦克风超过支持数量");
    }
    let mut endpoints = Vec::new();
    for index in 0..count {
        let device = unsafe { collection.Item(index)? };
        let volume: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None)? };
        endpoints.push(Endpoint {
            id: take_string(unsafe { device.GetId()? })?,
            name: friendly_name(&device)
                .map_err(|error| log::debug!("麦克风名称读取失败：{error:#}"))
                .ok(),
            muted: unsafe { volume.GetMute()?.as_bool() },
        });
    }
    Ok(endpoints)
}
pub(crate) fn set_muted(id: &str, muted: bool) -> Result<()> {
    let _apartment = Apartment::enter()?;
    let enumerator = enumerator()?;
    let id: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
    let device = unsafe { enumerator.GetDevice(PCWSTR(id.as_ptr()))? };
    let endpoint: IMMEndpoint = device.cast()?;
    if unsafe { endpoint.GetDataFlow()? } != eCapture
        || unsafe { device.GetState()? } != DEVICE_STATE_ACTIVE
    {
        bail!("麦克风已断开或身份不再是活动录音端点");
    }
    let volume: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None)? };
    unsafe { volume.SetMute(muted, &windows::core::GUID::zeroed())? };
    if unsafe { volume.GetMute()?.as_bool() } != muted {
        bail!("麦克风静音回读不一致");
    }
    Ok(())
}
