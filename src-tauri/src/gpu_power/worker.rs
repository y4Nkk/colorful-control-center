use super::{
    execution::Execution,
    native,
    operation::Io,
    request::{self, Invocation},
};
use crate::{controller_lease::ControllerLease, hal::oem_dchu::Dchu};
use anyhow::Result;

struct NativeIo {
    dchu: Dchu,
}
impl Io for NativeIo {
    fn inspect(&self, id: &str) -> Result<super::Device> {
        native::inspect(&self.dchu, id)
    }
    fn firmware(&self, enabled: bool) -> Result<()> {
        self.dchu
            .exchange_package(4, &native::protocol::firmware_packet(enabled))?;
        Ok(())
    }
    fn node(&self, id: &str, enabled: bool) -> Result<()> {
        native::set_node(id, enabled)
    }
    fn save(&self, automatic_power_off: bool) -> Result<()> {
        self.dchu
            .write_settings(1, 5, &[u8::from(!automatic_power_off)])
    }
    fn pause(&self) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
fn execute(invocation: &Invocation) -> Result<()> {
    invocation.request()?;
    crate::hal::authorization::require_elevated()?;
    let _lease = ControllerLease::retain(crate::controller_lease::NAME)?;
    let _worker =
        ControllerLease::try_acquire(concat!("Global\\", env!("CARGO_PKG_NAME"), ".gpu-worker"))?
            .ok_or_else(|| anyhow::anyhow!("已有独显授权工作进程"))?;
    request::serve(invocation, |request| {
        let io = NativeIo {
            dchu: Dchu::load()?,
        };
        Ok(Box::new(Execution::new(io, request)))
    })
}
pub fn run_child(args: &[String]) -> u32 {
    let invocation = match Invocation::from_args(args) {
        Ok(invocation) => invocation,
        Err(error) => {
            eprintln!("[oem-gpu-power] {error:#}");
            return 1;
        }
    };
    match execute(&invocation) {
        Ok(()) => 0,
        Err(error) => {
            if let Err(saved) = crate::hal::authorization::session::report_failure(
                &invocation.session_dir,
                &format!("{error:#}"),
            ) {
                eprintln!("[oem-gpu-power] 保存诊断失败：{saved:#}");
            }
            eprintln!("[oem-gpu-power] {error:#}");
            1
        }
    }
}
