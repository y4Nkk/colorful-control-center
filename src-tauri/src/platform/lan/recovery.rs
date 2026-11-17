use super::{
    model::*,
    store::{self, Record},
    transaction::{Io, Plan},
};
use anyhow::{Result, bail};
use std::{cell::RefCell, path::Path};

pub struct Prepared<'a, T> {
    pub io: &'a T,
    pub path: &'a Path,
    pub record: &'a RefCell<Record>,
}
impl<T: Io> Prepared<'_, T> {
    pub fn report(&self, error: Option<String>) -> Result<()> {
        let mut current = self.record.borrow_mut();
        let mut next = current.clone();
        next.last_error = error;
        store::write(self.path, &current, &next)?;
        *current = next;
        Ok(())
    }
}
impl<T: Io> Io for Prepared<'_, T> {
    fn inspect(&self, instance: &str) -> Result<Device> {
        self.io.inspect(instance)
    }
    fn prepare(&self, expected: &Device, change: &Change) -> Result<()> {
        let mut current = self.record.borrow_mut();
        let mut next = current.clone();
        next.remember(&Plan {
            expected: expected.clone(),
            changes: vec![change.clone()],
        })?;
        next.last_error = None;
        store::write(self.path, &current, &next)?;
        *current = next;
        Ok(())
    }
    fn write(&self, identity: &Identity, field: Field, value: &Option<Value>) -> Result<()> {
        if store::read(self.path)? != *self.record.borrow() {
            bail!("LAN恢复文件已被外部修改，未继续写入系统");
        }
        self.io.write(identity, field, value)
    }
}
