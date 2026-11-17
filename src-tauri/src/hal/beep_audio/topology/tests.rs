use super::*;

fn node(name: &str, mute: bool, volume: bool, incoming: &[&str]) -> Part {
    Part {
        name: name.into(),
        mute,
        volume,
        incoming: incoming.iter().map(|id| (*id).into()).collect(),
        ..Part::default()
    }
}

#[test]
fn a_named_mute_without_the_original_volume_label_does_not_select_a_different_control() {
    let parts = BTreeMap::from([("root".into(), node("PC Beep", true, false, &[]))]);
    assert!(select("root", &parts).unwrap().is_none());
}

#[test]
fn volume_name_and_mute_can_belong_to_distinct_parts_of_the_same_original_group() {
    let parts = BTreeMap::from([
        ("root".into(), node("", false, false, &["mute"])),
        ("mute".into(), node("静音", true, false, &["volume"])),
        ("volume".into(), node("PC Beep", false, true, &[])),
    ]);
    assert_eq!(
        select("root", &parts).unwrap(),
        Some(Selection {
            name: "PC Beep".into(),
            mute: Some("mute".into()),
            volume: Some("volume".into())
        })
    );
}

#[test]
fn branches_and_sum_nodes_do_not_borrow_a_downstream_master_mute() {
    for sum in [false, true] {
        let mut root = node(
            "主音量",
            true,
            true,
            if sum { &["beep"] } else { &["other", "beep"] },
        );
        root.sum = sum;
        let parts = BTreeMap::from([
            ("root".into(), root),
            ("other".into(), node("其他", true, true, &[])),
            ("beep".into(), node("PC Beep", false, true, &[])),
        ]);
        let selected = select("root", &parts).unwrap().unwrap();
        assert_eq!(selected.volume.as_deref(), Some("beep"));
        assert!(selected.mute.is_none());
    }
}

#[test]
fn first_matching_group_and_upstream_overwrite_follow_original_traversal_order() {
    let parts = BTreeMap::from([
        ("root".into(), node("", false, false, &["first", "second"])),
        ("first".into(), node("PC Beep", true, true, &["last-mute"])),
        ("last-mute".into(), node("", true, false, &[])),
        ("second".into(), node("PC Beep", true, true, &[])),
    ]);
    let selected = select("root", &parts).unwrap().unwrap();
    assert_eq!(selected.volume.as_deref(), Some("first"));
    assert_eq!(selected.mute.as_deref(), Some("last-mute"));
}

#[test]
fn terminal_connectors_are_not_traversed_and_invalid_topologies_do_not_create_a_target() {
    let mut terminal = node("", false, false, &["missing"]);
    terminal.terminal = true;
    assert!(
        select("root", &BTreeMap::from([("root".into(), terminal)]))
            .unwrap()
            .is_none()
    );
    let missing = BTreeMap::from([("root".into(), node("", false, false, &["missing"]))]);
    assert!(select("root", &missing).is_err());
    let cycle = BTreeMap::from([("root".into(), node("PC Beep", true, true, &["root"]))]);
    assert!(select("root", &cycle).is_err());
}
