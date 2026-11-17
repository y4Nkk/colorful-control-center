use super::Device;
use crate::{
    gpuctl::turbo::{mode, source},
    powerctl::mode::links,
};
use anyhow::{Result, bail};

impl source::SourceIo for Device {
    fn read(&self) -> Result<source::Source> {
        bail!("隔离基础档案未注入GPU调频来源");
    }
}
impl links::Source for Device {
    fn policy(&self) -> Result<mode::Policy> {
        Ok(mode::test_policy())
    }
    fn on_ac(&self) -> Result<Option<bool>> {
        Ok(Some(true))
    }
}
