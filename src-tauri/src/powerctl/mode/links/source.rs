use crate::{
    gpuctl::turbo::{mode, source},
    hal::Hal,
};
use anyhow::Result;

pub(crate) trait Source: source::SourceIo {
    fn policy(&self) -> Result<mode::Policy>;
    fn on_ac(&self) -> Result<Option<bool>>;
}
#[cfg(test)]
impl source::SourceIo for &dyn Source {
    fn read(&self) -> Result<source::Source> {
        (**self).read()
    }
}
#[cfg(test)]
impl Source for &dyn Source {
    fn policy(&self) -> Result<mode::Policy> {
        (**self).policy()
    }
    fn on_ac(&self) -> Result<Option<bool>> {
        (**self).on_ac()
    }
}
pub(super) struct Native<'a>(pub &'a Hal);
impl source::SourceIo for Native<'_> {
    fn read(&self) -> Result<source::Source> {
        source::SourceIo::read(&source::Native(self.0))
    }
}
impl Source for Native<'_> {
    #[cfg(not(test))]
    fn on_ac(&self) -> Result<Option<bool>> {
        source::supply()
    }
    #[cfg(test)]
    fn on_ac(&self) -> Result<Option<bool>> {
        anyhow::bail!("隔离模式未注入供电来源")
    }
    fn policy(&self) -> Result<mode::Policy> {
        let model = self.0.with_dchu(crate::hal::oem_identity::read)?;
        mode::read(self.0, &model)
    }
}
