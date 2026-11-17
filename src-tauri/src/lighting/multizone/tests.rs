use super::*;
use std::sync::Mutex;

struct Data {
    snapshot: Snapshot,
    sent: Vec<[u8; 37]>,
    saves: usize,
    reads: usize,
    fail_send: Vec<usize>,
    fail_save: Vec<usize>,
    fail_read: Vec<usize>,
}
struct Memory(Mutex<Data>);
impl Io for Memory {
    fn snapshot(&self) -> Result<Snapshot> {
        let mut data = self.0.lock().unwrap();
        data.reads += 1;
        if data.fail_read.contains(&data.reads) {
            bail!("隔离读取失败");
        }
        data.snapshot.validate()?;
        Ok(data.snapshot.clone())
    }
    fn ensure_available(&self) -> Result<()> {
        Ok(())
    }
    fn send(&self, packet: &[u8; 37]) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.sent.push(*packet);
        if data.fail_send.contains(&data.sent.len()) {
            bail!("隔离发送部分失败");
        }
        Ok(())
    }
    fn save(&self, patches: &[Patch]) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.saves += 1;
        let fail = data.fail_save.contains(&data.saves);
        for patch in patches {
            data.snapshot.page[patch.offset..patch.offset + patch.data.len()]
                .copy_from_slice(&patch.data);
            if fail {
                bail!("隔离保存部分失败");
            }
        }
        Ok(())
    }
}
fn setup() -> Memory {
    let mut page = vec![9; 256];
    page[32] = 1;
    page[34] = 3;
    page[35] = 4;
    page[80] = 1;
    Memory(Mutex::new(Data {
        snapshot: Snapshot {
            capabilities: Capabilities {
                keyboard_type: 23,
                light_bar_switchable: true,
                light_bar_enabled: true,
                cover: true,
            },
            page,
        },
        sent: vec![],
        saves: 0,
        reads: 0,
        fail_send: vec![],
        fail_save: vec![],
        fail_read: vec![],
    }))
}
fn request(io: &Memory) -> Request {
    let before = io.snapshot().unwrap();
    let mut values = before.values().unwrap();
    values.mode = 4;
    values.speed = 2;
    values.zones[3] = Rgb { r: 1, g: 2, b: 3 };
    Request {
        expected_revision: before.revision(),
        target: Target {
            capabilities: before.capabilities,
            values,
        },
    }
}

#[test]
fn all_modes_and_brightness_match_original_packet_layout() {
    let io = setup();
    let mut snapshot = io.snapshot().unwrap();
    for (mode, command) in [6, 255, 5, 7, 4, 1, 2, 3].into_iter().enumerate() {
        for level in 1..=4 {
            snapshot.page[32] = mode as u8;
            snapshot.page[35] = level;
            let packet = snapshot.packet().unwrap();
            assert_eq!(&packet[..3], &[44, command, 3]);
            assert_eq!(packet[16], [47, 95, 143, 191][usize::from(level - 1)]);
            assert_eq!(packet[32], [31, 46, 87, 128][usize::from(level - 1)]);
            assert_eq!(packet[36], packet[16]);
            assert!(packet[3..16].iter().all(|value| *value == 0));
        }
    }
}
#[test]
fn off_keeps_saved_level_and_colors_but_zeroes_all_outputs() {
    let io = setup();
    let before = io.snapshot().unwrap();
    let mut target = request(&io).target;
    target.values.enabled = false;
    let after = before.target(&target).unwrap();
    let packet = after.packet().unwrap();
    assert_eq!([packet[16], packet[32], packet[36]], [0, 0, 0]);
    assert_eq!(after.page[35], 4);
    assert_eq!(&packet[26..29], &[1, 2, 3]);
}
#[test]
fn inactive_lightbar_color_is_preserved_in_packet_and_save() {
    let io = setup();
    io.0.lock().unwrap().snapshot.capabilities.light_bar_enabled = false;
    let before = io.snapshot().unwrap();
    let target = request(&io).target;
    assert!(target.values.light_bar.is_none());
    let after = before.target(&target).unwrap();
    assert_eq!(&after.packet().unwrap()[29..32], &[9, 9, 9]);
    assert_eq!(after.packet().unwrap()[32], 0);
    assert!(
        after
            .patches(&before)
            .unwrap()
            .iter()
            .all(|patch| patch.offset != 27)
    );
}
#[test]
fn apply_and_initial_restore_preserve_every_unowned_byte() {
    let io = setup();
    let before = io.snapshot().unwrap();
    let mut baseline = None;
    let mut dirty = false;
    apply(&io, &mut baseline, &request(&io), &mut dirty).unwrap();
    io.0.lock().unwrap().snapshot.page[36] = 1;
    io.0.lock().unwrap().snapshot.page[100] = 7;
    restore(&io, baseline.as_ref().unwrap()).unwrap();
    let after = io.snapshot().unwrap();
    assert_eq!(after.revision(), before.revision());
    assert_eq!((after.page[36], after.page[100]), (1, 7));
}
#[test]
fn stale_revision_invalid_target_and_capabilities_send_nothing() {
    for change in 0..3 {
        let io = setup();
        let mut wanted = request(&io);
        match change {
            0 => wanted.expected_revision = "stale".into(),
            1 => wanted.target.values.speed = 4,
            _ => wanted.target.capabilities.cover = false,
        }
        let mut baseline = None;
        assert!(apply(&io, &mut baseline, &wanted, &mut false).is_err());
        assert!(baseline.is_none());
        assert!(io.0.lock().unwrap().sent.is_empty());
    }
}
#[test]
fn each_send_save_and_readback_failure_compensates() {
    for failure in 0..3 {
        let io = setup();
        let wanted = request(&io);
        let before = io.0.lock().unwrap().snapshot.revision();
        {
            let mut data = io.0.lock().unwrap();
            match failure {
                0 => data.fail_send = vec![1],
                1 => data.fail_save = vec![1],
                _ => {
                    let next = data.reads + 2;
                    data.fail_read = vec![next];
                }
            }
        }
        let mut baseline = None;
        let mut dirty = false;
        assert!(apply(&io, &mut baseline, &wanted, &mut dirty).is_err());
        assert!(!dirty);
        assert_eq!(io.0.lock().unwrap().snapshot.revision(), before);
        assert_eq!(io.0.lock().unwrap().sent.len(), 2);
    }
}
#[test]
fn failed_recovery_keeps_baseline_and_retry_replays_instant_packet() {
    let io = setup();
    io.0.lock().unwrap().fail_send = vec![1, 2];
    let mut baseline = None;
    let mut dirty = false;
    assert!(apply(&io, &mut baseline, &request(&io), &mut dirty).is_err());
    assert!(dirty && baseline.is_some());
    restore(&io, baseline.as_ref().unwrap()).unwrap();
    assert_eq!(io.0.lock().unwrap().sent.len(), 3);
}
#[test]
fn unknown_current_values_allow_only_known_initial_restore() {
    let io = setup();
    let before = io.snapshot().unwrap();
    io.0.lock().unwrap().snapshot.page[32] = 255;
    let current = io.snapshot().unwrap();
    let profile = current.profile(&Some(before.clone()), None).unwrap();
    assert!(profile.current.is_none() && profile.can_restore);
    restore(&io, &before).unwrap();
    assert_eq!(io.snapshot().unwrap().revision(), before.revision());
}
#[test]
fn platform_and_cover_gates_are_independent_and_strict() {
    let mut flags = [0; 24];
    flags[0] = 1;
    assert!(native::decode(&flags, None).unwrap().light_bar_enabled);
    flags[23] = 160;
    assert!(native::decode(&flags, Some(0)).unwrap().cover);
    assert!(!native::decode(&flags, Some(0)).unwrap().light_bar_enabled);
    assert!(native::decode(&flags, Some(2)).is_err());
    flags[0] = 0;
    assert!(native::decode(&flags, Some(1)).is_err());
}
#[test]
fn target_rejects_old_fields_and_out_of_range_values() {
    let io = setup();
    let target = request(&io).target;
    for speed in [0, 4, 255] {
        let mut invalid = target.clone();
        invalid.values.speed = speed;
        assert!(invalid.validate().is_err());
    }
    let mut invalid = serde_json::to_value(target).unwrap();
    invalid["legacyZones"] = true.into();
    assert!(serde_json::from_value::<Target>(invalid).is_err());
}

#[test]
fn capability_change_blocks_new_writes_without_discarding_initial_target() {
    let io = setup();
    let before = io.snapshot().unwrap();
    io.0.lock().unwrap().snapshot.capabilities.cover = false;
    let current = io.snapshot().unwrap();
    let profile = current.profile(&Some(before.clone()), None).unwrap();
    assert!(!profile.can_restore && profile.blocked_reason.is_some());
    assert!(profile.initial.is_some());
    assert!(restore(&io, &before).is_err());
    assert!(io.0.lock().unwrap().sent.is_empty());
}
