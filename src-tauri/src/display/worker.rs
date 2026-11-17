use super::{guardian, native};
use anyhow::Result;
pub fn run_child() -> Result<()> {
    let _lease = crate::controller_lease::ControllerLease::retain(crate::controller_lease::NAME)?;
    let input = guardian::protocol::read(std::io::stdin());
    guardian::protocol::run(input, &native::Native, |message| {
        use std::io::Write;
        let mut output = std::io::stdout().lock();
        if let Ok(text) = serde_json::to_string(&message) {
            let _ = writeln!(output, "{text}");
            let _ = output.flush();
        }
    })
}
