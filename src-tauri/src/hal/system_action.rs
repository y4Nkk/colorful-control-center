use anyhow::Result;
use serde::Serialize;

#[cfg(not(test))]
mod native;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Kind {
    Sleep,
    Hibernate,
    ScreenOff,
    Shutdown,
}
impl Kind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Sleep => "睡眠",
            Self::Hibernate => "休眠",
            Self::ScreenOff => "关闭屏幕",
            Self::Shutdown => "关机",
        }
    }
}
pub(crate) fn modern_standby() -> Result<bool> {
    #[cfg(test)]
    {
        anyhow::bail!("隔离测试未注入现代待机能力");
    }
    #[cfg(not(test))]
    {
        native::modern_standby()
    }
}
pub(crate) fn request(kind: Kind) -> Result<()> {
    #[cfg(test)]
    {
        let _ = kind;
        anyhow::bail!("隔离测试不请求真实系统电源动作");
    }
    #[cfg(not(test))]
    {
        native::request(kind)
    }
}
