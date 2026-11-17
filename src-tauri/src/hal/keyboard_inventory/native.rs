use anyhow::{Context, Result, bail};
use windows::{
    Win32::System::{
        Variant::{VARIANT, VT_BSTR},
        Wmi::{
            IWbemClassObject, WBEM_FLAG_NONSYSTEM_ONLY, WBEM_S_FALSE, WBEM_S_NO_MORE_DATA,
            WBEM_S_TIMEDOUT,
        },
    },
    core::{BSTR, Interface},
};

struct Enumeration(IWbemClassObject);
impl Drop for Enumeration {
    fn drop(&mut self) {
        if let Err(error) = unsafe { self.0.EndEnumeration() } {
            log::warn!("[hotkeys] 结束键盘属性枚举失败：{error}");
        }
    }
}

fn object_has_usb(object: &IWbemClassObject) -> Result<bool> {
    unsafe { object.BeginEnumeration(WBEM_FLAG_NONSYSTEM_ONLY.0)? };
    let _enumeration = Enumeration(object.clone());
    for _ in 0..512 {
        let mut value = VARIANT::default();
        // 生成的Result包装丢弃成功HRESULT；此处必须区分属性结束标记。
        let status = unsafe {
            (object.vtable().Next)(
                object.as_raw(),
                0,
                std::ptr::null_mut(),
                &mut value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if status.0 == WBEM_S_NO_MORE_DATA.0 {
            return Ok(false);
        }
        status.ok()?;
        if value.vt() == VT_BSTR && BSTR::try_from(&value)?.to_string().contains("USB") {
            return Ok(true);
        }
    }
    bail!("键盘属性枚举未正常结束")
}

pub(super) fn usb_present() -> Result<bool> {
    let session = crate::hal::wmi::Session::connect("ROOT\\CIMV2")?;
    let objects = session.query("SELECT * FROM Win32_Keyboard")?;
    for _ in 0..128 {
        let mut next = [None];
        let mut count = 0;
        let status = unsafe { objects.Next(1000, &mut next, &mut count) };
        status.ok()?;
        if status.0 == WBEM_S_TIMEDOUT.0 {
            bail!("USB键盘清单读取超时");
        }
        if count == 0 && status.0 == WBEM_S_FALSE.0 {
            return Ok(false);
        }
        if count != 1 {
            bail!("USB键盘清单返回不完整");
        }
        if object_has_usb(next[0].as_ref().context("键盘清单缺少实例")?)? {
            return Ok(true);
        }
        if status.0 == WBEM_S_FALSE.0 {
            return Ok(false);
        }
    }
    bail!("USB键盘清单超过支持数量")
}
