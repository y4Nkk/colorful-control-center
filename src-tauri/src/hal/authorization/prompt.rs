//! 后台执行仅继承现有令牌；限制必须显式传递到工作线程。
use anyhow::{Result, bail};
use std::{cell::Cell, marker::PhantomData, rc::Rc};

thread_local! {
    static INTERACTIVE: Cell<bool> = const { Cell::new(true) };
}

pub(crate) struct Restriction {
    previous: bool,
    _thread: PhantomData<Rc<()>>,
}
impl Restriction {
    pub(crate) fn enter(interactive: bool) -> Self {
        let previous = INTERACTIVE.replace(INTERACTIVE.get() && interactive);
        Self {
            previous,
            _thread: PhantomData,
        }
    }
}
impl Drop for Restriction {
    fn drop(&mut self) {
        INTERACTIVE.set(self.previous);
    }
}
pub(crate) fn interactive() -> bool {
    INTERACTIVE.get()
}
pub(crate) fn verb(elevated: bool, interactive: bool) -> Result<&'static str> {
    match (elevated, interactive) {
        (true, _) => Ok("open"),
        (false, true) => Ok("runas"),
        (false, false) => bail!("后台操作需要管理员启动；请先确认管理员登录自启，未请求临时提权"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_cannot_be_loosened_and_restores_on_exit() {
        assert!(interactive());
        {
            let _outer = Restriction::enter(false);
            let _inner = Restriction::enter(true);
            assert!(!interactive());
        }
        assert!(interactive());
    }
    #[test]
    fn background_never_requests_uac() {
        assert!(verb(false, false).is_err());
        assert_eq!(verb(true, false).unwrap(), "open");
        assert_eq!(verb(false, true).unwrap(), "runas");
    }
    #[test]
    fn restriction_is_explicit_across_threads() {
        let _outer = Restriction::enter(false);
        let captured = interactive();
        assert!(
            !std::thread::spawn(move || {
                let _guard = Restriction::enter(captured);
                interactive()
            })
            .join()
            .unwrap()
        );
    }
}
