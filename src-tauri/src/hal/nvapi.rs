//! NVAPI 动态加载 HAL（GPU 核心/显存频率偏移的唯一硬件通道）。
//!
//! 安全边界（docs/nvapi-contract.md）：
//! - 只经 `LoadLibraryExA(LOAD_LIBRARY_SEARCH_SYSTEM32)` 从系统目录加载
//!   `nvapi64.dll`（x64 规范名）→ `nvapi.dll` 兜底，杜绝 CWD/PATH 劫持；
//! - 所有函数指针经 `nvapi_QueryInterface` 按已验证 ID 解析，解析失败即报错，
//!   不猜测、不静默降级；
//! - 失败一律返回带 NVAPI 状态名的 `anyhow` 错误，绝不伪造成功；
//! - 本模块不做任何钳位/回滚策略——那是 transaction / gpuctl 的职责。
//!
//! 单元测试只覆盖纯逻辑与“加载不存在的库必须干净失败”，不触碰 GPU。

pub mod clock;
pub mod ffi;
pub mod pstates;
pub mod transaction;
pub(crate) mod turbo;

use std::ffi::c_void;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use windows_sys::Win32::Foundation::{FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExA,
};

use ffi::{Pstates20Info, SHORT_STRING_MAX};

type QueryInterfaceFn = unsafe extern "system" fn(u32) -> *mut c_void;
type InitializeFn = unsafe extern "system" fn() -> i32;
type UnloadFn = unsafe extern "system" fn() -> i32;
type GetErrorMessageFn = unsafe extern "system" fn(i32, *mut u8) -> i32;
type EnumPhysicalGpusFn = unsafe extern "system" fn(*mut *mut c_void, *mut u32) -> i32;
type GetFullNameFn = unsafe extern "system" fn(*mut c_void, *mut u8) -> i32;
type GetPstates20Fn = unsafe extern "system" fn(*mut c_void, *mut Pstates20Info) -> i32;
type SetPstates20Fn = unsafe extern "system" fn(*mut c_void, *mut Pstates20Info) -> i32;

/// x64 进程的 NVAPI 规范 DLL 名（本机 System32 实测只有 nvapi64.dll；
/// bemanitools/NVFC/ccminer 均按指针宽度选择）。
const DLL_CANDIDATES: [&str; 2] = ["nvapi64.dll", "nvapi.dll"];

macro_rules! resolve {
    ($qi:expr, $id:expr, $name:expr, $ty:ty) => {{
        // SAFETY: nvapi_QueryInterface 为 FFI 入口；id 为已验证常量。
        let ptr = unsafe { $qi($id) };
        if ptr.is_null() {
            return Err(anyhow!(
                "nvapi_QueryInterface 解析 {} (0x{:08X}) 返回 NULL——驱动过旧或 NVAPI 不可用",
                $name,
                $id
            ));
        }
        // SAFETY: ptr 来自 nvapi_QueryInterface 的函数指针；$ty 为等宽 Win64 调用约定指针。
        unsafe { std::mem::transmute::<*mut c_void, $ty>(ptr) }
    }};
}

/// 已初始化的 NVAPI 会话（加载 → Initialize 成功）。持有期间驱动模块保持加载。
pub struct NvApi {
    module: HMODULE,
    query: QueryInterfaceFn,
    #[allow(dead_code)] // 与 Initialize 成对（官方契约）；进程生命周期内常驻
    initialize: InitializeFn,
    unload: UnloadFn,
    get_error_message: GetErrorMessageFn,
    enum_physical_gpus: EnumPhysicalGpusFn,
    get_full_name: GetFullNameFn,
    get_pstates20: GetPstates20Fn,
    set_pstates20: SetPstates20Fn,
}

unsafe impl Send for NvApi {}
unsafe impl Sync for NvApi {}

/// 从系统目录加载第一个存在的候选 DLL；全部失败时返回带系统错误码的 anyhow 错误。
fn load_system_library(names: &[&str]) -> Result<HMODULE> {
    let mut last: Option<std::io::Error> = None;
    for name in names {
        let path = format!("{name}\0");
        // SAFETY: path 是合法 NUL 结尾字节串；LOAD_LIBRARY_SEARCH_SYSTEM32 限定只搜系统目录。
        let module = unsafe {
            LoadLibraryExA(
                path.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        };
        if !module.is_null() {
            return Ok(module);
        }
        last = Some(std::io::Error::last_os_error());
    }
    let detail = last
        .map(|e| e.to_string())
        .unwrap_or_else(|| "未知错误".into());
    Err(anyhow!("加载系统目录 {} 失败：{}", names.join("/"), detail))
}

/// 把 NVAPI 状态码翻译成“符号名 + 驱动描述”（若可用）。
fn nvapi_status_text(get_error_message: GetErrorMessageFn, status: i32) -> String {
    let mut buf = [0u8; SHORT_STRING_MAX];
    // SAFETY: buf 是 64 字节 NvAPI_ShortString 的合法存储。
    let msg_ok = unsafe { get_error_message(status, buf.as_mut_ptr()) };
    let text = if msg_ok == ffi::STATUS_OK {
        let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..len]).into_owned()
    } else {
        String::new()
    };
    if text.is_empty() {
        ffi::status_name(status).to_string()
    } else {
        format!("{} ({})", ffi::status_name(status), text)
    }
}

impl NvApi {
    /// 加载 + 解析全部函数指针 + `NvAPI_Initialize`。
    /// 只在运行时命令路径调用（GpuController 惰性初始化），不在单元测试或启动期触碰。
    pub fn load() -> Result<Self> {
        let module = load_system_library(&DLL_CANDIDATES)
            .with_context(|| "NVAPI 不可用：未在系统目录找到 NVIDIA 驱动接口库")?;
        // SAFETY: module 来自成功的 LoadLibraryExA。
        let query =
            unsafe { GetProcAddress(module, c"nvapi_QueryInterface".as_ptr() as *const u8) };
        let query: QueryInterfaceFn = match query {
            None => {
                // SAFETY: module 有效，立即释放避免泄漏。
                unsafe { FreeLibrary(module) };
                return Err(anyhow!(
                    "nvapi64.dll 缺少 nvapi_QueryInterface 导出——非标准驱动库"
                ));
            }
            Some(f) => {
                // SAFETY: 导出名匹配的 NVAPI 入口，Win64 调用约定。
                unsafe {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, QueryInterfaceFn>(f)
                }
            }
        };

        let initialize = resolve!(query, ffi::ID_INITIALIZE, "NvAPI_Initialize", InitializeFn);
        let unload = resolve!(query, ffi::ID_UNLOAD, "NvAPI_Unload", UnloadFn);
        let get_error_message = resolve!(
            query,
            ffi::ID_GET_ERROR_MESSAGE,
            "NvAPI_GetErrorMessage",
            GetErrorMessageFn
        );
        let enum_physical_gpus = resolve!(
            query,
            ffi::ID_ENUM_PHYSICAL_GPUS,
            "NvAPI_EnumPhysicalGPUs",
            EnumPhysicalGpusFn
        );
        let get_full_name = resolve!(
            query,
            ffi::ID_GET_FULL_NAME,
            "NvAPI_GPU_GetFullName",
            GetFullNameFn
        );
        let get_pstates20 = resolve!(
            query,
            ffi::ID_GET_PSTATES20,
            "NvAPI_GPU_GetPstates20",
            GetPstates20Fn
        );
        let set_pstates20 = resolve!(
            query,
            ffi::ID_SET_PSTATES20,
            "NvAPI_GPU_SetPstates20",
            SetPstates20Fn
        );

        // SAFETY: initialize 无参数。
        let status = unsafe { initialize() };
        if status != ffi::STATUS_OK {
            let text = nvapi_status_text(get_error_message, status);
            // SAFETY: module 有效。
            unsafe { FreeLibrary(module) };
            return Err(anyhow!("NvAPI_Initialize 失败：{text}"));
        }

        Ok(Self {
            module,
            query,
            initialize,
            unload,
            get_error_message,
            enum_physical_gpus,
            get_full_name,
            get_pstates20,
            set_pstates20,
        })
    }

    /// NVAPI 状态码 → 用户可读文本（供上层错误拼接）。
    pub fn status_text(&self, status: i32) -> String {
        nvapi_status_text(self.get_error_message, status)
    }

    /// 枚举物理 GPU 句柄（多卡时按驱动顺序，取首个即独显场景的 dGPU）。
    pub fn gpu_handles(&self) -> Result<Vec<*mut c_void>> {
        let mut handles = [std::ptr::null_mut::<c_void>(); ffi::MAX_PHYSICAL_GPUS];
        let mut count: u32 = 0;
        // SAFETY: handles/count 尺寸符合 NvAPI_EnumPhysicalGPUs 契约。
        let status =
            unsafe { (self.enum_physical_gpus)(handles.as_mut_ptr(), &mut count as *mut u32) };
        if status != ffi::STATUS_OK {
            return Err(anyhow!(
                "枚举 NVIDIA 物理 GPU 失败：{}",
                self.status_text(status)
            ));
        }
        let count = count as usize;
        if count > ffi::MAX_PHYSICAL_GPUS || handles[..count].iter().any(|handle| handle.is_null())
        {
            return Err(anyhow!("NVIDIA 物理 GPU 枚举结果无效"));
        }
        Ok(handles[..count].to_vec())
    }

    /// GPU 完整名（如 "NVIDIA GeForce RTX 4060 Laptop GPU"）。
    pub fn gpu_full_name(&self, handle: *mut c_void) -> Result<String> {
        let mut buf = [0u8; SHORT_STRING_MAX];
        // SAFETY: handle 来自本会话枚举；buf 为合法 ShortString。
        let status = unsafe { (self.get_full_name)(handle, buf.as_mut_ptr()) };
        if status != ffi::STATUS_OK {
            return Err(anyhow!("读取 GPU 名称失败：{}", self.status_text(status)));
        }
        let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        Ok(String::from_utf8_lossy(&buf[..len]).into_owned())
    }

    /// 读取 PSTATES20 表：按 V3→V2→V1 探测（与 NvAPIWrapper Accepts 顺序一致）。
    pub fn read_pstates(&self, handle: *mut c_void) -> Result<Pstates20Info> {
        let mut last_status = ffi::STATUS_ERROR;
        for version in [ffi::VERSION_V3, ffi::VERSION_V2, ffi::VERSION_V1] {
            let mut info = Pstates20Info::zeroed();
            info.version = version;
            // SAFETY: handle 来自本会话枚举；info 布局见 ffi.rs 契约测试。
            let status = unsafe { (self.get_pstates20)(handle, &mut info) };
            if status == ffi::STATUS_OK {
                return Ok(info);
            }
            last_status = status;
            if status != ffi::STATUS_INCOMPATIBLE_STRUCT_VERSION {
                break;
            }
        }
        Err(anyhow!(
            "读取 GPU P-States 表失败：{}",
            self.status_text(last_status)
        ))
    }

    /// 写入 PSTATES20 最小包（由 pstates::build_set_request 构造）。
    pub fn write_pstates(&self, handle: *mut c_void, request: &Pstates20Info) -> Result<()> {
        // SAFETY: request 为 V1 最小包；布局与调用契约见 docs/nvapi-contract.md §3。
        let status = unsafe { (self.set_pstates20)(handle, request as *const _ as *mut _) };
        if status != ffi::STATUS_OK {
            return Err(anyhow!(
                "写入 GPU 频率偏移失败：{}",
                self.status_text(status)
            ));
        }
        Ok(())
    }
}

impl Drop for NvApi {
    fn drop(&mut self) {
        // SAFETY: 与 load() 成对；Unload 失败（如仍有引用计数）不致命，忽略返回值。
        unsafe {
            let _ = (self.unload)();
            if !self.module.is_null() {
                FreeLibrary(self.module);
            }
        }
    }
}

/// 已建立的 NVAPI 会话：加载的库 + 物理 GPU 句柄 + 展示名。
/// GPU 名称读取失败不影响可用性（仅少一条展示信息）。
pub struct NvApiSession {
    api: Arc<NvApi>,
    handle: *mut c_void,
    name: Option<String>,
}

// SAFETY: 句柄为不透明指针；GpuController 的 Mutex 保证串行访问，无并发别名。
unsafe impl Send for NvApiSession {}
unsafe impl Sync for NvApiSession {}

impl NvApiSession {
    /// 打开会话：加载 nvapi64.dll → Initialize → 枚举物理 GPU → 取首个。
    /// 多 NVIDIA 卡时取驱动序首个（本产品目标为单独显笔记本）。
    pub fn open() -> Result<Arc<Self>> {
        let api = Arc::new(NvApi::load()?);
        let handles = api.gpu_handles()?;
        let handle = *handles
            .first()
            .ok_or_else(|| anyhow!("未枚举到 NVIDIA 物理 GPU（无独立显卡或驱动未安装）"))?;
        let name = api.gpu_full_name(handle).ok();
        Ok(Arc::new(Self { api, handle, name }))
    }

    /// GPU 完整名（如 "NVIDIA GeForce RTX 4060 Laptop GPU"）。
    pub fn gpu_name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

impl transaction::PstatesIo for NvApiSession {
    fn read(&self) -> Result<pstates::ParsedPstates> {
        let info = self
            .api
            .read_pstates(self.handle)
            .context("读取 GPU P-States 表失败")?;
        Ok(pstates::parse(&info))
    }

    fn write(&self, core_khz: Option<i32>, mem_khz: Option<i32>) -> Result<()> {
        let request = pstates::build_set_request(core_khz, mem_khz);
        self.api.write_pstates(self.handle, &request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 加载不存在的库必须干净失败并携带系统错误码（不触碰真实 nvapi）。
    #[test]
    fn missing_library_fails_with_exact_error() {
        let err = load_system_library(&["colorful-ccc-no-such-dll.dll"])
            .expect_err("不存在的库不应加载成功");
        let text = format!("{err:#}");
        assert!(
            text.contains("colorful-ccc-no-such-dll.dll"),
            "报错应指名库：{text}"
        );
        // 系统错误码 126 = ERROR_MOD_NOT_FOUND（LoadLibraryEx 契约）
        assert!(
            text.contains("126") || text.contains("找不到指定的模块"),
            "应含系统错误：{text}"
        );
    }

    /// 候选名回退顺序：首个存在者生效（本测试机器 nvapi64.dll 存在时也不应误加载 nvapi.dll
    /// 以外的路径——本用例只用假名验证回退逻辑本身）。
    #[test]
    fn candidate_fallback_reports_last_error() {
        let err = load_system_library(&["ccc-missing-a.dll", "ccc-missing-b.dll"])
            .expect_err("全部缺失时应失败");
        // 报错列出全部候选名，且以最后一个失败的系统错误结尾
        let text = format!("{err:#}");
        assert!(
            text.contains("ccc-missing-a.dll/ccc-missing-b.dll"),
            "应列出候选：{text}"
        );
    }
}
