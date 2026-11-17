use anyhow::{Result, bail};
use serde::Serialize;
use std::time::Duration;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub title: String,
    pub detail: String,
    pub error: bool,
    pub level: Option<Level>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Level {
    pub value: u8,
    pub max: u8,
}
impl Message {
    pub fn result(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            error: false,
            level: None,
        }
    }
    fn validate(&self) -> Result<()> {
        if self.title.is_empty()
            || self.title.chars().count() > 64
            || self.detail.chars().count() > 512
        {
            bail!("屏显内容长度无效");
        }
        if self
            .level
            .as_ref()
            .is_some_and(|level| level.max == 0 || level.value > level.max)
        {
            bail!("屏显比例无效");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub sequence: u64,
    pub revision: u64,
    pub message: Option<Message>,
    pub visible: bool,
    pub error: Option<String>,
}
#[derive(Default)]
pub struct Model {
    pub snapshot: Snapshot,
    deadline: Option<Duration>,
    pending: bool,
}
impl Model {
    pub fn pending_sequence(&self) -> Option<u64> {
        self.pending.then_some(self.snapshot.sequence)
    }
    pub fn post(&mut self, message: Message) -> Result<u64> {
        message.validate()?;
        let sequence = self
            .snapshot
            .sequence
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("屏显序号耗尽"))?;
        self.snapshot = Snapshot {
            sequence,
            revision: self.snapshot.revision + 1,
            message: Some(message),
            visible: false,
            error: None,
        };
        self.deadline = None;
        self.pending = true;
        Ok(sequence)
    }
    pub fn shown(&mut self, sequence: u64, at: Duration) -> Option<Duration> {
        if self.pending_sequence() != Some(sequence) {
            return None;
        }
        self.pending = false;
        self.snapshot.visible = true;
        self.snapshot.revision += 1;
        let deadline = at + Duration::from_millis(2800);
        self.deadline = Some(deadline);
        Some(deadline)
    }
    pub fn expire(&mut self, sequence: u64, at: Duration) -> bool {
        if sequence != self.snapshot.sequence || self.deadline.is_none_or(|deadline| at < deadline)
        {
            return false;
        }
        self.snapshot.visible = false;
        self.snapshot.revision += 1;
        self.deadline = None;
        true
    }
    pub fn fail(&mut self, sequence: u64, error: String) {
        if sequence == self.snapshot.sequence {
            self.snapshot.error = Some(error);
            self.pending = false;
            self.snapshot.revision += 1;
            self.snapshot.visible = false;
            self.deadline = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadlines_start_after_show_and_old_tasks_cannot_change_new_message() {
        let mut model = Model::default();
        let first = model.post(Message::result("模式", "性能")).unwrap();
        assert!(!model.expire(first, Duration::from_secs(100)));
        model.shown(first, Duration::from_secs(100)).unwrap();
        assert!(model.shown(first, Duration::from_secs(101)).is_none());
        let second = model.post(Message::result("风扇", "自动")).unwrap();
        assert!(model.shown(first, Duration::from_secs(101)).is_none());
        model.fail(first, "旧错误".into());
        assert!(model.snapshot.error.is_none());
        model.shown(second, Duration::from_secs(110)).unwrap();
        assert!(!model.expire(first, Duration::from_secs(120)));
        assert!(!model.expire(second, Duration::from_millis(112799)));
        assert!(model.expire(second, Duration::from_millis(112800)));
        assert!(!model.snapshot.visible);
    }
    #[test]
    fn invalid_message_does_not_replace_current_and_errors_are_retained() {
        let mut model = Model::default();
        let first = model.post(Message::result("屏显预览", "示例")).unwrap();
        let mut invalid = Message::result("", "");
        assert!(model.post(invalid.clone()).is_err());
        invalid.title = "比例".into();
        invalid.level = Some(Level { value: 2, max: 1 });
        assert!(model.post(invalid).is_err());
        assert_eq!(model.snapshot.sequence, first);
        model.fail(first, "窗口创建失败".into());
        assert_eq!(model.snapshot.error.as_deref(), Some("窗口创建失败"));
    }
}
