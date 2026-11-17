use super::*;
use ccc_core::keymap::{Action, Binding, DeviceSettings, Step, compile::Item};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Default)]
struct Memory {
    collections: BTreeMap<usize, Vec<Item>>,
    devices: BTreeSet<usize>,
    next: usize,
    calls: Vec<&'static str>,
    registered: Vec<(usize, Vec<Item>, Vec<Item>)>,
    fail: Option<(&'static str, usize)>,
    last_error: i32,
}
impl Memory {
    fn fail(&mut self, name: &'static str) -> bool {
        self.calls.push(name);
        if let Some((kind, nth)) = &mut self.fail {
            if *kind == name {
                *nth -= 1;
                if *nth == 0 {
                    self.fail = None;
                    self.last_error = 9;
                    return true;
                }
            }
        }
        false
    }
}
thread_local! { static MEMORY:RefCell<Memory>=RefCell::new(Memory::default()); }
fn memory<T>(f: impl FnOnce(&mut Memory) -> T) -> T {
    MEMORY.with(|m| f(&mut m.borrow_mut()))
}
unsafe extern "C" fn create() -> Handle {
    memory(|m| {
        if m.fail("create") {
            return std::ptr::null_mut();
        }
        m.next += 1;
        let id = m.next + 100;
        m.collections.insert(id, vec![]);
        id as Handle
    })
}
unsafe extern "C" fn delete(handle: Handle) -> i32 {
    memory(|m| {
        m.calls.push("delete");
        assert!(m.collections.remove(&(handle as usize)).is_some());
    });
    0
}
unsafe extern "C" fn open(device: i32) -> Handle {
    memory(|m| {
        if m.fail("open") {
            return std::ptr::null_mut();
        }
        let id = device as usize + 1;
        assert!(m.devices.insert(id));
        id as Handle
    })
}
unsafe extern "C" fn close(handle: Handle) -> i32 {
    memory(|m| {
        m.calls.push("close");
        assert!(m.devices.remove(&(handle as usize)));
    });
    0
}
unsafe extern "C" fn enable(_: Handle, enabled: i32) -> i32 {
    memory(|m| i32::from(m.fail(if enabled == 0 { "disable" } else { "enable" })))
}
unsafe extern "C" fn clear(_: Handle) -> i32 {
    memory(|m| i32::from(m.fail("clear")))
}
fn append(handle: Handle, item: Item) -> i32 {
    memory(|m| {
        if m.fail("append") {
            return 1;
        }
        m.collections
            .get_mut(&(handle as usize))
            .unwrap()
            .push(item);
        0
    })
}
unsafe extern "C" fn add_key(
    handle: Handle,
    kind: i32,
    code: u16,
    make_delay_ms: u32,
    break_delay_ms: u32,
) -> i32 {
    append(
        handle,
        Item::Click {
            kind,
            code,
            make_delay_ms,
            break_delay_ms,
        },
    )
}
unsafe extern "C" fn down(handle: Handle, code: u16, delay_ms: u32) -> i32 {
    append(handle, Item::Down { code, delay_ms })
}
unsafe extern "C" fn up(handle: Handle, code: u16, delay_ms: u32) -> i32 {
    append(handle, Item::Up { code, delay_ms })
}
unsafe extern "C" fn text(handle: Handle, value: *const u16, make: u32, release: u32) -> i32 {
    assert_eq!([make, release], [0, 0]);
    let mut result = vec![];
    let mut offset = 0;
    loop {
        let code = unsafe { *value.add(offset) };
        result.push(code);
        if code == 0 {
            break;
        }
        offset += 1;
    }
    append(handle, Item::Text(result))
}
unsafe extern "C" fn register(device: Handle, hotkey: Handle, actions: Handle) -> i32 {
    memory(|m| {
        assert!(!hotkey.is_null());
        assert!(!actions.is_null());
        m.registered.push((
            device as usize,
            m.collections[&(hotkey as usize)].clone(),
            m.collections[&(actions as usize)].clone(),
        ));
        i32::from(m.fail("register"))
    })
}
unsafe extern "C" fn error() -> i32 {
    memory(|m| m.last_error)
}
fn library() -> Library {
    memory(|m| *m = Memory::default());
    Library {
        handle: std::ptr::null_mut(),
        api: Api {
            create,
            delete,
            open,
            close,
            enable,
            clear,
            add_key,
            down,
            up,
            text,
            register,
            error,
        },
    }
}
fn configuration() -> Configuration {
    Configuration {
        devices: vec![
            DeviceSettings {
                device: Device::Keyboard,
                enabled: true,
                bindings: vec![Binding {
                    trigger: 30,
                    action: Action::Keys {
                        steps: vec![
                            Step::Key {
                                scan_code: 0xe01d,
                                pressed: true,
                                delay_after_ms: 20,
                            },
                            Step::Key {
                                scan_code: 0xe01d,
                                pressed: false,
                                delay_after_ms: 30,
                            },
                        ],
                    },
                }],
            },
            DeviceSettings {
                device: Device::Mouse,
                enabled: false,
                bindings: vec![Binding {
                    trigger: 1,
                    action: Action::Disable,
                }],
            },
        ],
    }
}
fn assert_released() {
    memory(|m| {
        assert!(m.devices.is_empty());
        assert!(m.collections.is_empty());
    })
}
#[test]
fn original_zero_success_keeps_scan_codes_delays_aliases_and_nonnull_disabled_collection() {
    let library = library();
    library.replace(&configuration()).unwrap();
    memory(|m| {
        assert_eq!(m.registered.len(), 4);
        assert_eq!(m.registered[0].0, 1);
        assert_eq!(m.registered[3].0, 2);
        assert_eq!(
            m.registered[0].2,
            vec![
                Item::Down {
                    code: 0xe01d,
                    delay_ms: 20
                },
                Item::Up {
                    code: 0xe01d,
                    delay_ms: 30
                }
            ]
        );
        assert_eq!(
            m.registered[1].1[0],
            Item::Click {
                kind: 0,
                code: 0xe077,
                make_delay_ms: 0,
                break_delay_ms: 0
            }
        );
        assert_eq!(
            m.registered[2].1[0],
            Item::Click {
                kind: 0,
                code: 0xe078,
                make_delay_ms: 0,
                break_delay_ms: 0
            }
        );
        assert!(m.registered[3].2.is_empty());
    });
    assert_released();
}
#[test]
fn text_ffi_is_utf16_terminated_and_events_keep_the_header_key_type() {
    let library = library();
    let mut cfg = configuration();
    cfg.devices.truncate(1);
    cfg.devices[0].bindings[0].action = Action::Text {
        text: "灯🌀".into(),
        before: vec![],
        after: vec![],
    };
    library.replace(&cfg).unwrap();
    memory(|m| {
        assert_eq!(
            m.registered[0].2,
            vec![Item::Text(vec![0x706f, 0xd83c, 0xdf00, 0])]
        )
    });
    assert_released();
}
#[test]
fn preparation_failure_on_either_device_is_before_any_filter_mutation_and_releases_every_handle() {
    for (operation, nth) in [
        ("open", 1),
        ("open", 2),
        ("create", 1),
        ("create", 5),
        ("append", 1),
        ("append", 6),
    ] {
        let library = library();
        memory(|m| m.fail = Some((operation, nth)));
        assert!(library.replace(&configuration()).is_err());
        memory(|m| {
            assert!(
                m.calls
                    .iter()
                    .all(|c| !["disable", "clear", "register", "enable"].contains(c))
            )
        });
        assert_released();
    }
}
#[test]
fn registration_failure_is_not_hidden_by_later_collection_cleanup_or_enabling() {
    let library = library();
    memory(|m| m.fail = Some(("register", 2)));
    let failure = library.replace(&configuration()).unwrap_err().to_string();
    assert!(failure.contains("注册按键映射"));
    assert!(failure.contains("HKFltrLib=1"));
    memory(|m| {
        assert_eq!(m.registered.len(), 2);
        assert!(!m.calls.contains(&"enable"));
    });
    assert_released();
}
#[test]
fn invalid_configuration_never_opens_a_device() {
    let library = library();
    let mut cfg = configuration();
    cfg.devices[0].bindings[0].trigger = 0;
    assert!(library.replace(&cfg).is_err());
    memory(|m| assert!(m.calls.is_empty()));
}
#[test]
fn all_header_errors_have_actionable_status_and_zero_is_not_a_failure() {
    checked("隔离", 0).unwrap();
    for status in 1..=11 {
        assert!(
            checked("隔离", status)
                .unwrap_err()
                .to_string()
                .contains(&format!("HKFltrLib={status}"))
        );
    }
}
