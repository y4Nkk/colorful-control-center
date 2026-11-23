//! NVAPI FFI 布局与接口 ID（唯一事实见 docs/nvapi-contract.md）。
//!
//! 全部 ID 与结构布局经三路独立来源交叉验证：
//! - NVIDIA 官方 nvapi.h（github.com/NVIDIA/nvapi，NV_GPU_PERF_PSTATES20_INFO_V1/V2、
//!   NvApi_Status、NV_GPU_PUBLIC_CLOCK_ID）
//! - falahati/NvAPIWrapper（FunctionId.cs / PerformanceStates20*.cs）
//! - graphitemaster/NVFC（src/nvapi.h/.cpp）、djhackersdev/bemanitools（src/main/nv/module.c）、
//!   firoorg/ccminer（nvapi.cpp）
//!
//! 布局断言测试见本文件底部；测试不加载 DLL、不触碰硬件。

/// NvAPI_ShortString 长度（官方 nvapi_lite_common.h）。
pub const SHORT_STRING_MAX: usize = 64;
/// NvAPI_EnumPhysicalGPUs 句柄数组容量（官方 nvapi.h）。
pub const MAX_PHYSICAL_GPUS: usize = 64;

// ---------------------------------------------------------------------------
// QueryInterface 接口 ID（nvapi_QueryInterface 以 id 查询函数指针）
// ---------------------------------------------------------------------------

/// NvAPI_Initialize
pub const ID_INITIALIZE: u32 = 0x0150E828;
/// NvAPI_Unload
pub const ID_UNLOAD: u32 = 0xD22BDD7E;
/// NvAPI_GetErrorMessage
pub const ID_GET_ERROR_MESSAGE: u32 = 0x6C2D048C;
/// NvAPI_EnumPhysicalGPUs
pub const ID_ENUM_PHYSICAL_GPUS: u32 = 0xE5AC921F;
/// NvAPI_GPU_GetFullName
pub const ID_GET_FULL_NAME: u32 = 0xCEEE8E9F;
/// NvAPI_GPU_GetPstates20（官方 nvapi.h 有函数声明）
pub const ID_GET_PSTATES20: u32 = 0x6FF81213;
/// NvAPI_GPU_SetPstates20（私有接口；曾流传的 0x0F4DA7CF / 0x08595393 均为错误值，
/// 正确值 0x0F4DAE6B 被 NvAPIWrapper/NVFC/bemanitools/ccminer/jNizM 一致采用）
pub const ID_SET_PSTATES20: u32 = 0x0F4DAE6B;

// ---------------------------------------------------------------------------
// NvAPI_Status（官方 nvapi_lite_common.h，仅保留本域用到的码）
// ---------------------------------------------------------------------------

pub const STATUS_OK: i32 = 0;
pub const STATUS_ERROR: i32 = -1;
pub const STATUS_LIBRARY_NOT_FOUND: i32 = -2;
pub const STATUS_NO_IMPLEMENTATION: i32 = -3;
pub const STATUS_API_NOT_INITIALIZED: i32 = -4;
pub const STATUS_INVALID_ARGUMENT: i32 = -5;
pub const STATUS_NVIDIA_DEVICE_NOT_FOUND: i32 = -6;
pub const STATUS_END_ENUMERATION: i32 = -7;
pub const STATUS_INVALID_HANDLE: i32 = -8;
pub const STATUS_INCOMPATIBLE_STRUCT_VERSION: i32 = -9;
pub const STATUS_HANDLE_INVALIDATED: i32 = -10;
pub const STATUS_NOT_SUPPORTED: i32 = -104;
pub const STATUS_INVALID_USER_PRIVILEGE: i32 = -137;

/// 把状态码翻译为 NVAPI 符号名（错误信息直报，不伪造成功）。
pub fn status_name(status: i32) -> &'static str {
    match status {
        STATUS_OK => "NVAPI_OK",
        STATUS_ERROR => "NVAPI_ERROR",
        STATUS_LIBRARY_NOT_FOUND => "NVAPI_LIBRARY_NOT_FOUND",
        STATUS_NO_IMPLEMENTATION => "NVAPI_NO_IMPLEMENTATION",
        STATUS_API_NOT_INITIALIZED => "NVAPI_API_NOT_INITIALIZED",
        STATUS_INVALID_ARGUMENT => "NVAPI_INVALID_ARGUMENT",
        STATUS_NVIDIA_DEVICE_NOT_FOUND => "NVAPI_NVIDIA_DEVICE_NOT_FOUND",
        STATUS_END_ENUMERATION => "NVAPI_END_ENUMERATION",
        STATUS_INVALID_HANDLE => "NVAPI_INVALID_HANDLE",
        STATUS_INCOMPATIBLE_STRUCT_VERSION => "NVAPI_INCOMPATIBLE_STRUCT_VERSION",
        STATUS_HANDLE_INVALIDATED => "NVAPI_HANDLE_INVALIDATED",
        STATUS_NOT_SUPPORTED => "NVAPI_NOT_SUPPORTED",
        STATUS_INVALID_USER_PRIVILEGE => "NVAPI_INVALID_USER_PRIVILEGE",
        _ => "NVAPI_<unknown>",
    }
}

// ---------------------------------------------------------------------------
// PSTATES20 域常量（官方 nvapi.h NV_GPU_PUBLIC_CLOCK_ID / NV_GPU_PERF_PSTATE_ID）
// ---------------------------------------------------------------------------

/// 3D 图形（核心）时钟域
pub const CLOCK_GRAPHICS: u32 = 0;
/// 显存时钟域
pub const CLOCK_MEMORY: u32 = 4;
/// P0 = 3D Performance 最高性能态（偏移读写的目标 pstate）
pub const PSTATE_P0: u32 = 0;
/// 时钟条目 typeId：单频（写入包固定用 0，同 LenovoLegionToolkit / g-helper）
pub const CLOCK_TYPE_SINGLE: u32 = 0;

pub const MAX_PSTATES: usize = 16;
pub const MAX_CLOCKS: usize = 8;
pub const MAX_BASE_VOLTAGES: usize = 4;

/// NVAPI 版本字段编码：`sizeof(struct) | (ver << 16)`（官方 MAKE_NVAPI_VERSION）。
pub const fn make_version(size: u32, ver: u32) -> u32 {
    size | (ver << 16)
}

// ---------------------------------------------------------------------------
// NV_GPU_PERF_PSTATES20_INFO（官方 nvapi.h L1134–L1331；字段名对齐官方）
// ---------------------------------------------------------------------------

/// 参数增量：value 为当前增量，valueRange 为驱动允许的增量区间（单位随条目：kHz / uV）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParamDelta {
    pub value: i32,
    pub min: i32,
    pub max: i32,
}

/// 单个时钟条目（44 字节）。`data` 是官方 C 联合体的平铺视图：
/// single{freq_kHz} | range{minFreq,maxFreq,domainId,minVoltage,maxVoltage}。
/// 读取路径不解释它，写入路径不携带它 → 用 `[u32; 5]` 保真透传。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ClockEntry {
    /// NV_GPU_PUBLIC_CLOCK_ID（0=GRAPHICS, 4=MEMORY）
    pub domain_id: u32,
    /// 0=SINGLE, 1=RANGE
    pub type_id: u32,
    /// bit0 = bIsEditable
    pub flags: u32,
    /// 当前频率增量（kHz）
    pub freq_delta_khz: ParamDelta,
    /// C 联合体平铺（20 字节）
    pub data: [u32; 5],
}

/// 单个基础电压条目（24 字节）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BaseVoltageEntry {
    /// NV_GPU_PERF_VOLTAGE_INFO_DOMAIN_ID（0=CORE）
    pub domain_id: u32,
    /// bit0 = bIsEditable
    pub flags: u32,
    /// 当前电压（uV）
    pub volt_uv: u32,
    /// 电压增量（uV）
    pub volt_delta_uv: ParamDelta,
}

/// 单个 pstate（456 字节）。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PStateEntry {
    /// NV_GPU_PERF_PSTATE_ID（P0=0）
    pub pstate_id: u32,
    /// bit0 = bIsEditable
    pub flags: u32,
    pub clocks: [ClockEntry; MAX_CLOCKS],
    pub base_voltages: [BaseVoltageEntry; MAX_BASE_VOLTAGES],
}

/// NV_GPU_PERF_PSTATES20_INFO：V1/V2/V3 同体（V2 起尾部追加 ov 段）。
/// 读取用 V3→V2→V1 探测；写入固定构造 V1 最小包。总大小 7416 字节（V2/V3），
/// V1 调用只触及前 7316 字节。
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Pstates20Info {
    pub version: u32,
    /// bit0 = bIsEditable（顶层 OC 能力）
    pub flags: u32,
    pub num_pstates: u32,
    pub num_clocks: u32,
    pub num_base_voltages: u32,
    pub pstates: [PStateEntry; MAX_PSTATES],
    /// V2/V3 尾段：过压设置（读取透传，写入不携带）
    pub num_ov_voltages: u32,
    pub ov_voltages: [BaseVoltageEntry; MAX_BASE_VOLTAGES],
}

impl Pstates20Info {
    /// 全零初始化（POD；仅用于随后显式填充字段的写入包构造）。
    ///
    /// SAFETY: 结构体只含整数与 POD 数组，全零是合法值。
    pub fn zeroed() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

/// V1（7316 字节）版本字段：写入包固定用它。
pub const VERSION_V1: u32 = make_version(7316, 1);
/// V2（7416 字节）版本字段。
pub const VERSION_V2: u32 = make_version(7416, 2);
/// V3（结构同 V2）版本字段，现代驱动读取首选。
pub const VERSION_V3: u32 = make_version(7416, 3);

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{size_of, size_of_val};
    use std::ptr::addr_of;

    /// 布局契约：与官方 nvapi.h 声明逐字段对齐（docs/nvapi-contract.md §2）。
    #[test]
    fn ffi_layout_matches_official_header() {
        assert_eq!(size_of::<ParamDelta>(), 12);
        assert_eq!(size_of::<BaseVoltageEntry>(), 24);
        assert_eq!(size_of::<ClockEntry>(), 44);
        assert_eq!(size_of::<PStateEntry>(), 456);
        assert_eq!(size_of::<Pstates20Info>(), 7416);

        // ClockEntry 字段偏移（联合体 data 从 24 起，占 20 字节）
        let entry = ClockEntry::default();
        assert_eq!(
            addr_of!(entry.domain_id) as usize - addr_of!(entry) as usize,
            0
        );
        assert_eq!(
            addr_of!(entry.type_id) as usize - addr_of!(entry) as usize,
            4
        );
        assert_eq!(addr_of!(entry.flags) as usize - addr_of!(entry) as usize, 8);
        assert_eq!(
            addr_of!(entry.freq_delta_khz) as usize - addr_of!(entry) as usize,
            12
        );
        assert_eq!(addr_of!(entry.data) as usize - addr_of!(entry) as usize, 24);

        // 顶层字段偏移：version 0, flags 4, numPstates 8, numClocks 12, numBaseVoltages 16,
        // pstates[0] 20, num_ov_voltages 20+7296, ov_voltages 7320
        let info = Pstates20Info::zeroed();
        let base = addr_of!(info) as usize;
        assert_eq!(addr_of!(info.flags) as usize - base, 4);
        assert_eq!(addr_of!(info.num_pstates) as usize - base, 8);
        assert_eq!(addr_of!(info.num_clocks) as usize - base, 12);
        assert_eq!(addr_of!(info.num_base_voltages) as usize - base, 16);
        assert_eq!(addr_of!(info.pstates) as usize - base, 20);
        assert_eq!(addr_of!(info.num_ov_voltages) as usize - base, 7316);
        assert_eq!(addr_of!(info.ov_voltages) as usize - base, 7320);
        assert_eq!(size_of_val(&info.pstates), 7296);
        assert_eq!(size_of_val(&info.ov_voltages), 96);
    }

    /// 版本常量必须等于 `sizeof | ver<<16` 编码（Get/Set 的 ABI 门面）。
    #[test]
    fn version_constants_encode_size_and_version() {
        assert_eq!(VERSION_V1, 0x0001_1C94); // 7316 | 1<<16
        assert_eq!(VERSION_V2, 0x0002_1CF8); // 7416 | 2<<16
        assert_eq!(VERSION_V3, 0x0003_1CF8); // 7416 | 3<<16
    }

    /// 已验证的接口 ID 快照：任何一个被无意改动都会在此失败。
    #[test]
    fn verified_interface_ids() {
        assert_eq!(ID_INITIALIZE, 0x0150E828);
        assert_eq!(ID_UNLOAD, 0xD22BDD7E);
        assert_eq!(ID_GET_ERROR_MESSAGE, 0x6C2D048C);
        assert_eq!(ID_ENUM_PHYSICAL_GPUS, 0xE5AC921F);
        assert_eq!(ID_GET_FULL_NAME, 0xCEEE8E9F);
        assert_eq!(ID_GET_PSTATES20, 0x6FF81213);
        assert_eq!(ID_SET_PSTATES20, 0x0F4DAE6B);
        // 公开时钟域 / pstate 常量（官方 NV_GPU_PUBLIC_CLOCK_ID）
        assert_eq!((CLOCK_GRAPHICS, CLOCK_MEMORY, PSTATE_P0), (0, 4, 0));
    }

    #[test]
    fn status_names_cover_used_codes() {
        assert_eq!(status_name(0), "NVAPI_OK");
        assert_eq!(status_name(-9), "NVAPI_INCOMPATIBLE_STRUCT_VERSION");
        assert_eq!(status_name(-137), "NVAPI_INVALID_USER_PRIVILEGE");
        assert_eq!(status_name(-7777), "NVAPI_<unknown>");
    }
}
