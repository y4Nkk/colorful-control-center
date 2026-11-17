use super::*;
pub(super) struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
impl Key {
    pub fn open(driver: &str, writable: bool) -> Result<Self> {
        let path = wide(&format!(
            "SYSTEM\\CurrentControlSet\\Control\\Class\\{driver}"
        ));
        let mut key = std::ptr::null_mut();
        checked(unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                path.as_ptr(),
                0,
                KEY_READ | if writable { KEY_SET_VALUE } else { 0 },
                &mut key,
            )
        })?;
        Ok(Self(key))
    }
    pub fn read(&self, name: &str) -> Result<Option<Value>> {
        let name = wide(name);
        let (mut kind, mut size) = (0, 0);
        let code = unsafe {
            RegQueryValueExW(
                self.0,
                name.as_ptr(),
                std::ptr::null(),
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        checked(code)?;
        if size > 4096 {
            bail!("LAN驱动字段过大");
        }
        let mut bytes = vec![0; size as usize];
        let expected_kind = kind;
        checked(unsafe {
            RegQueryValueExW(
                self.0,
                name.as_ptr(),
                std::ptr::null(),
                &mut kind,
                bytes.as_mut_ptr(),
                &mut size,
            )
        })?;
        if size as usize > bytes.len() || kind != expected_kind {
            bail!("LAN字段读取期间变化");
        }
        bytes.truncate(size as usize);
        Ok(Some(Value { kind, bytes }))
    }
    pub fn signature(&self) -> Result<(Option<Value>, Option<Value>, Option<Value>)> {
        Ok((
            self.read("NetCfgInstanceId")?,
            self.read("DriverVersion")?,
            self.read("MatchingDeviceId")?,
        ))
    }
    pub fn write(&self, field: Field, value: &Option<Value>) -> Result<()> {
        if !field.writable() {
            bail!("LAN字段只读");
        }
        let name = wide(field.name());
        let code = if let Some(value) = value {
            value.validate()?;
            unsafe {
                RegSetValueExW(
                    self.0,
                    name.as_ptr(),
                    0,
                    value.kind,
                    value.bytes.as_ptr(),
                    value.bytes.len() as u32,
                )
            }
        } else {
            let result = unsafe { RegDeleteValueW(self.0, name.as_ptr()) };
            if result == ERROR_FILE_NOT_FOUND {
                0
            } else {
                result
            }
        };
        checked(code)
    }
}
