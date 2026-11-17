use super::*;
use anyhow::{Result, bail};
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
        Ole::OleRun,
    },
    core::BSTR,
};
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
fn connect() -> Result<Cui> {
    let unknown: IUnknown = unsafe { CoCreateInstance(&CLASS, None, CLSCTX_ALL) }?;
    unsafe { OleRun(&unknown) }?;
    Ok(unknown.cast()?)
}
fn uid(cui: &Cui) -> Result<u32> {
    let adapter = BSTR::from(r"\\.\DISPLAY1");
    for index in 0..256 {
        let (mut uid, mut kind, mut status) = (0, 0, 0);
        let result = unsafe {
            (cui.vtable().enumerate)(
                cui.as_raw(),
                adapter.as_ptr().cast_mut().cast(),
                index,
                &mut uid,
                &mut kind,
                &mut status,
            )
        };
        result.ok()?;
        if result.0 == 0 {
            return Ok(uid);
        }
    }
    bail!("CUI未枚举到原厂DISPLAY1适配器")
}
fn in_sta(operation: impl FnOnce(&Cui) -> Result<u32> + Send + 'static) -> Result<u32> {
    std::thread::Builder::new()
        .name("panel-cui-sta".into())
        .spawn(move || {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok()?;
            let _apartment = Apartment;
            let cui = connect()?;
            operation(&cui)
        })?
        .join()
        .map_err(|_| anyhow::anyhow!("面板CUI线程异常"))?
}
pub(crate) fn identify() -> Result<u32> {
    in_sta(uid)
}
pub(crate) fn repair(expected: u32) -> Result<()> {
    in_sta(move |cui| {
        let actual = uid(cui)?;
        if actual != expected {
            bail!("NVSR适配器已变化，未发送请求");
        }
        let mut packet = packet(actual);
        let mut status = 0;
        unsafe {
            (cui.vtable().request)(
                cui.as_raw(),
                &REQUEST,
                packet.len() as u32,
                packet.as_mut_ptr(),
                &mut status,
            )
        }
        .ok()?;
        // 原厂以 HRESULT 判定调用；未定义的输出值只记录，不猜测成功码。
        log::info!("[panel-cui] NVSR request returned status={status:#x}");
        Ok(actual)
    })?;
    Ok(())
}
