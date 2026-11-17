#[cfg(not(test))]
use super::request;
use super::{Device, sequence::Sequence};
use crate::{hal::Hal, hal::authorization::session::Authorized};
use anyhow::Result;

pub(crate) trait Environment: Send + Sync {
    fn supported(&self, hal: &Hal) -> Result<bool>;
    fn list(&self, hal: &Hal) -> Result<Vec<Device>>;
    fn authorize(&self, sequence: Sequence) -> Result<Box<dyn Authorized>>;
}
#[cfg(not(test))]
pub(super) struct Native;
#[cfg(not(test))]
impl Environment for Native {
    fn supported(&self, hal: &Hal) -> Result<bool> {
        hal.with_dchu(super::native::supported)
    }
    fn list(&self, hal: &Hal) -> Result<Vec<Device>> {
        hal.with_dchu(super::native::list)
    }
    fn authorize(&self, sequence: Sequence) -> Result<Box<dyn Authorized>> {
        request::prepare(sequence)
    }
}
