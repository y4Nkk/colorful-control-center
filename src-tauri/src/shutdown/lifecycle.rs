use std::sync::{Condvar, Mutex, MutexGuard};

struct State {
    requested: bool,
    operations: usize,
    exit_code: Option<i32>,
}

pub(crate) struct Lifecycle {
    state: Mutex<State>,
    changed: Condvar,
}

impl Lifecycle {
    pub(crate) const fn new() -> Self {
        Self {
            state: Mutex::new(State {
                requested: false,
                operations: 0,
                exit_code: None,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn enter(&self) -> Option<Operation<'_>> {
        let mut state = self.lock();
        if state.requested {
            return None;
        }
        state.operations += 1;
        Some(Operation(self))
    }

    pub(crate) fn request<E>(&self, commit: impl FnOnce() -> Result<(), E>) -> Result<bool, E> {
        let mut state = self.lock();
        if state.requested {
            return Ok(false);
        }
        commit()?;
        state.requested = true;
        self.changed.notify_all();
        Ok(true)
    }

    pub(crate) fn closing(&self) -> bool {
        self.lock().requested
    }

    pub(crate) fn wait_for_request(&self) {
        drop(
            self.changed
                .wait_while(self.lock(), |state| !state.requested)
                .unwrap_or_else(|error| error.into_inner()),
        );
    }

    pub(crate) fn drain(&self) {
        drop(
            self.changed
                .wait_while(self.lock(), |state| state.operations != 0)
                .unwrap_or_else(|error| error.into_inner()),
        );
    }

    pub(crate) fn complete(&self, code: i32) {
        self.lock().exit_code = Some(code);
        self.changed.notify_all();
    }

    pub(crate) fn exit_code(&self) -> Option<i32> {
        self.lock().exit_code
    }

    pub(crate) fn wait_for_completion(&self) -> i32 {
        self.changed
            .wait_while(self.lock(), |state| state.exit_code.is_none())
            .unwrap_or_else(|error| error.into_inner())
            .exit_code
            .expect("退出收尾已完成")
    }
}

pub(crate) struct Operation<'a>(&'a Lifecycle);

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.operations -= 1;
        if state.operations == 0 {
            self.0.changed.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_commit_failure_leaves_admission_open_and_existing_exit_prevents_commit() {
        let lifecycle = Lifecycle::new();
        assert_eq!(
            lifecycle.request(|| Err("child exited")),
            Err("child exited")
        );
        assert!(lifecycle.enter().is_some());
        assert_eq!(lifecycle.request(|| Ok::<(), &str>(())), Ok(true));
        assert_eq!(
            lifecycle.request::<&str>(|| panic!("exit already requested")),
            Ok(false)
        );
        assert!(lifecycle.enter().is_none());
    }
}
