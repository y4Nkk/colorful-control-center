//! AuxSwitchCC30.dll 的固定 CUI ABI；COM 对象与 STA 由同一线程拥有。
use std::ffi::c_void;
use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, Interface};
#[cfg(not(test))]
const CLASS: GUID = GUID::from_u128(0x7160a13d_73da_4cea_95b9_37356478588a);
const REQUEST: GUID = GUID::from_u128(0xbfb9816c_aeb0_434b_99f3_0f94e6bebf0d);
#[repr(transparent)]
#[derive(Clone)]
struct Cui(IUnknown);
unsafe impl Interface for Cui {
    type Vtable = Vtable;
    const IID: GUID = GUID::from_u128(0xf932c038_6484_45ca_8fa1_7c8c279f7aee);
}
#[repr(C)]
struct Vtable {
    base: IUnknown_Vtbl,
    before_enum: [usize; 9],
    enumerate: unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        u32,
        *mut u32,
        *mut u32,
        *mut u32,
    ) -> HRESULT,
    before_request: [usize; 31],
    request: unsafe extern "system" fn(*mut c_void, *const GUID, u32, *mut u8, *mut u32) -> HRESULT,
}
#[cfg(not(test))]
mod native;
#[cfg(not(test))]
pub(crate) use native::{identify, repair};
fn packet(uid: u32) -> [u8; 148] {
    let mut bytes = [0u8; 148];
    for (index, value) in [uid, 8, 132, 0x721].into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_cui_slots_match_oem_x64_abi() {
        assert_eq!(std::mem::offset_of!(Vtable, enumerate), 0x60);
        assert_eq!(std::mem::offset_of!(Vtable, request), 0x160);
        assert_eq!(std::mem::size_of::<Cui>(), std::mem::size_of::<IUnknown>());
    }
    #[test]
    fn exact_original_request_has_fixed_header_and_zero_payload() {
        let packet = packet(0x12345678);
        assert_eq!(
            &packet[..16],
            &[
                0x78, 0x56, 0x34, 0x12, 8, 0, 0, 0, 132, 0, 0, 0, 0x21, 7, 0, 0
            ]
        );
        assert_eq!(packet.len(), 148);
        assert!(packet[16..].iter().all(|byte| *byte == 0));
        assert_eq!(
            REQUEST,
            GUID::from_u128(0xbfb9816c_aeb0_434b_99f3_0f94e6bebf0d)
        );
    }
}
