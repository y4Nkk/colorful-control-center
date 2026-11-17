use super::{authorization::Authorized, model::Frame, worker::Operation};
use crate::hal::Hal;
use anyhow::Result;

pub(crate) trait Environment: Send + Sync {
    fn frame(&self, hal: &Hal, recovering: bool) -> Result<Frame>;
    fn ensure_owner_absent(&self) -> Result<()>;
    fn authorize(&self, operation: Operation) -> Result<Box<dyn Authorized>>;
}
#[cfg(not(test))]
pub(super) struct Native;
#[cfg(not(test))]
impl Environment for Native {
    fn frame(&self, hal: &Hal, recovering: bool) -> Result<Frame> {
        hal.with_dchu(|io| super::native::frame(io, recovering))
    }
    fn ensure_owner_absent(&self) -> Result<()> {
        crate::hal::oem_owner::ensure_owner_absent()
    }
    fn authorize(&self, operation: Operation) -> Result<Box<dyn Authorized>> {
        super::authorization::prepare(operation)
    }
}
