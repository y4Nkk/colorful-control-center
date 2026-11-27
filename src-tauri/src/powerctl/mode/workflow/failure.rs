use anyhow::{Error, Result};
use std::fmt;

#[derive(Debug)]
pub(in crate::powerctl::mode) struct Failure {
    cause: Error,
    recovery_attempted: bool,
    recovery_errors: Vec<String>,
}

impl From<Error> for Failure {
    fn from(cause: Error) -> Self {
        Self {
            cause,
            recovery_attempted: false,
            recovery_errors: Vec::new(),
        }
    }
}

impl Failure {
    pub(in crate::powerctl::mode) fn record_recovery(&mut self, stage: &str, result: Result<()>) {
        self.recovery_attempted = true;
        self.record_error(stage, result);
    }

    pub(super) fn record_error(&mut self, stage: &str, result: Result<()>) {
        if let Err(error) = result {
            self.recovery_errors.push(format!("{stage}：{error:#}"));
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let recovery = if !self.recovery_errors.is_empty() {
            Err(self.recovery_errors.join("；"))
        } else if self.recovery_attempted {
            Ok(())
        } else {
            return write!(f, "{:#}", self.cause);
        };
        f.write_str(&ccc_core::msr::with_recovery(
            format!("{:#}", self.cause),
            recovery,
        ))
    }
}
