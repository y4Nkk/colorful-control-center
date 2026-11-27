//! AppSettings 字段事务：预读全部旧值，写入并回读，失败恢复全部目标字段。
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Patch {
    pub offset: usize,
    pub data: Vec<u8>,
}

impl Patch {
    pub fn new(offset: usize, data: impl Into<Vec<u8>>) -> Self {
        Self {
            offset,
            data: data.into(),
        }
    }
}

pub trait SettingsAccess {
    fn read(&self, page: u8, offset: usize, length: usize) -> Result<Vec<u8>, String>;
    fn write(&self, page: u8, offset: usize, data: &[u8]) -> Result<(), String>;
}

pub fn apply(
    io: &impl SettingsAccess,
    page: u8,
    patches: &[Patch],
    trigger: Option<&Patch>,
) -> Result<Vec<Patch>, String> {
    validate(patches, trigger)?;
    let capture = |p: &Patch| {
        let data = io.read(page, p.offset, p.data.len())?;
        if data.len() != p.data.len() {
            return Err("AppSettings 返回长度不足".into());
        }
        Ok(Patch::new(p.offset, data))
    };
    let before = patches
        .iter()
        .map(capture)
        .collect::<Result<Vec<_>, String>>()?;
    let before_trigger = trigger.map(capture).transpose()?;
    if let Err(error) = write_fields(io, page, patches, trigger) {
        return Err(crate::msr::with_recovery(
            error,
            write_fields(io, page, &before, before_trigger.as_ref()),
        ));
    }
    Ok(before)
}

/// 写前快照和失败补偿由调用方事务持有。
pub fn write_verified(
    io: &impl SettingsAccess,
    page: u8,
    patches: &[Patch],
    trigger: Option<&Patch>,
) -> Result<(), String> {
    validate(patches, trigger)?;
    write_fields(io, page, patches, trigger)
}

fn write_fields(
    io: &impl SettingsAccess,
    page: u8,
    patches: &[Patch],
    trigger: Option<&Patch>,
) -> Result<(), String> {
    for patch in patches {
        io.write(page, patch.offset, &patch.data)?;
    }
    if let Some(flag) = trigger {
        io.write(page, flag.offset, &flag.data)?;
    }
    for patch in patches.iter().chain(trigger) {
        if io.read(page, patch.offset, patch.data.len())? != patch.data {
            return Err(format!(
                "AppSettings page{page} offset{} 回读不一致",
                patch.offset
            ));
        }
    }
    Ok(())
}

fn validate(patches: &[Patch], trigger: Option<&Patch>) -> Result<(), String> {
    if patches.is_empty() {
        return Err("没有提供设置变更".into());
    }
    let mut occupied = [false; 256];
    for patch in patches.iter().chain(trigger) {
        let end = patch
            .offset
            .checked_add(patch.data.len())
            .ok_or("字段范围溢出")?;
        if patch.data.is_empty() || end > 256 {
            return Err("字段超出 AppSettings 页面".into());
        }
        for byte in &mut occupied[patch.offset..end] {
            if *byte {
                return Err("AppSettings 字段重叠".into());
            }
            *byte = true;
        }
    }
    Ok(())
}

/// 只保留每个字段首次写入前的值，供用户恢复整个调校会话。
pub fn remember(baseline: &mut Vec<Patch>, before: Vec<Patch>) {
    for patch in before {
        if !baseline
            .iter()
            .any(|existing| existing.offset == patch.offset)
        {
            baseline.push(patch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    struct Mock {
        page: RefCell<[u8; 256]>,
        writes: Cell<usize>,
        fail_at: usize,
    }
    impl SettingsAccess for Mock {
        fn read(&self, _: u8, offset: usize, length: usize) -> Result<Vec<u8>, String> {
            Ok(self.page.borrow()[offset..offset + length].to_vec())
        }
        fn write(&self, _: u8, offset: usize, data: &[u8]) -> Result<(), String> {
            self.writes.set(self.writes.get() + 1);
            if self.writes.get() == self.fail_at {
                return Err("模拟写入失败".into());
            }
            self.page.borrow_mut()[offset..offset + data.len()].copy_from_slice(data);
            Ok(())
        }
    }
    #[test]
    fn partial_write_restores_both_fields_without_touching_other_bytes() {
        let io = Mock {
            page: RefCell::new([7; 256]),
            writes: Cell::new(0),
            fail_at: 2,
        };
        let error = apply(
            &io,
            6,
            &[Patch::new(33, [1, 2]), Patch::new(61, [3, 4])],
            None,
        )
        .unwrap_err();
        assert!(error.contains("已恢复"));
        assert_eq!(*io.page.borrow(), [7; 256]);
    }
    #[test]
    fn invalid_or_overlapping_fields_never_write() {
        let io = Mock {
            page: RefCell::new([0; 256]),
            writes: Cell::new(0),
            fail_at: 0,
        };
        assert!(apply(&io, 6, &[Patch::new(255, [1, 2])], None).is_err());
        assert!(apply(&io, 6, &[Patch::new(33, [1, 2]), Patch::new(34, [1])], None).is_err());
        assert_eq!(io.writes.get(), 0);
    }
    #[test]
    fn failed_trigger_restores_original_unapplied_flag() {
        let io = Mock {
            page: RefCell::new([0; 256]),
            writes: Cell::new(0),
            fail_at: 2,
        };
        assert!(apply(&io, 6, &[Patch::new(33, [80])], Some(&Patch::new(32, [1]))).is_err());
        assert_eq!(*io.page.borrow(), [0; 256]);
    }
    #[test]
    fn repeated_writes_keep_first_baseline() {
        let mut baseline = Vec::new();
        remember(&mut baseline, vec![Patch::new(33, [1])]);
        remember(
            &mut baseline,
            vec![Patch::new(33, [2]), Patch::new(61, [3])],
        );
        assert_eq!(baseline, vec![Patch::new(33, [1]), Patch::new(61, [3])]);
    }
}
