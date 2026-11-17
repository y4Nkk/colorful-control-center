use super::*;
use ccc_core::keymap::compile::{CompiledBinding, CompiledDevice, Item};

struct Collection<'a> {
    api: &'a Api,
    handle: Handle,
}
impl Drop for Collection<'_> {
    fn drop(&mut self) {
        if let Err(error) = checked("释放按键集合", unsafe {
            (self.api.delete)(self.handle)
        }) {
            log::error!("[key-filter] {error:#}");
        }
    }
}
impl<'a> Collection<'a> {
    fn prepare(api: &'a Api, items: &[Item]) -> Result<Self> {
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            let status = unsafe { (api.error)() };
            bail!("创建按键集合失败（HKFltrLib={status}）");
        }
        let collection = Self { api, handle };
        for item in items {
            let status = unsafe {
                match item {
                    Item::Click {
                        kind,
                        code,
                        make_delay_ms,
                        break_delay_ms,
                    } => (api.add_key)(handle, *kind, *code, *make_delay_ms, *break_delay_ms),
                    Item::Down { code, delay_ms } => (api.down)(handle, *code, *delay_ms),
                    Item::Up { code, delay_ms } => (api.up)(handle, *code, *delay_ms),
                    Item::Text(value) => (api.text)(handle, value.as_ptr(), 0, 0),
                }
            };
            checked("准备按键集合", status)?;
        }
        Ok(collection)
    }
}
struct Session<'a> {
    api: &'a Api,
    handle: Handle,
}
impl Drop for Session<'_> {
    fn drop(&mut self) {
        if let Err(error) = checked("关闭过滤设备", unsafe {
            (self.api.close)(self.handle)
        }) {
            log::error!("[key-filter] {error:#}");
        }
    }
}
impl<'a> Session<'a> {
    fn open(api: &'a Api, device: Device) -> Result<Self> {
        let handle = unsafe { (api.open)(device_code(device)) };
        if handle.is_null() {
            let status = unsafe { (api.error)() };
            bail!("打开{device:?}过滤设备失败（HKFltrLib={status}）");
        }
        Ok(Self { api, handle })
    }
}
struct Binding<'a> {
    actions: Collection<'a>,
    hotkeys: Vec<Collection<'a>>,
}
impl<'a> Binding<'a> {
    fn prepare(api: &'a Api, binding: &CompiledBinding) -> Result<Self> {
        Ok(Self {
            actions: Collection::prepare(api, &binding.actions)?,
            hotkeys: binding
                .hotkeys
                .iter()
                .map(|keys| Collection::prepare(api, keys))
                .collect::<Result<_>>()?,
        })
    }
}
struct Prepared<'a> {
    session: Session<'a>,
    enabled: bool,
    bindings: Vec<Binding<'a>>,
}
impl<'a> Prepared<'a> {
    fn prepare(api: &'a Api, plan: &CompiledDevice) -> Result<Self> {
        Ok(Self {
            session: Session::open(api, plan.device)?,
            enabled: plan.enabled,
            bindings: plan
                .bindings
                .iter()
                .map(|binding| Binding::prepare(api, binding))
                .collect::<Result<_>>()?,
        })
    }
    fn execute(&self) -> Result<()> {
        let api = self.session.api;
        let device = self.session.handle;
        checked("暂时停用过滤", unsafe { (api.enable)(device, 0) })?;
        checked("清除旧映射", unsafe { (api.clear)(device) })?;
        for binding in &self.bindings {
            for hotkey in &binding.hotkeys {
                // 禁用键使用非空的空动作集合；NULL在原厂协议中另有含义。
                checked("注册按键映射", unsafe {
                    (api.register)(device, hotkey.handle, binding.actions.handle)
                })?;
            }
        }
        checked("设置过滤启用状态", unsafe {
            (api.enable)(device, i32::from(self.enabled))
        })
    }
}
pub(super) fn replace(api: &Api, plan: &[CompiledDevice]) -> Result<()> {
    let prepared = plan
        .iter()
        .map(|device| Prepared::prepare(api, device))
        .collect::<Result<Vec<_>>>()?;
    for device in &prepared {
        device.execute()?;
    }
    Ok(())
}
