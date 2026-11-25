use super::*;
use crate::profiles::{ProfileSettings, check_revision, list_in, load_from, save_in};
use std::{fs::FileTimes, sync::Barrier, thread};

fn profile(name: &str, mode: u8) -> ProfileFile {
    ProfileFile {
        version: 1,
        name: name.into(),
        saved_at_ms: 0,
        settings: ProfileSettings {
            fan_mode: Some(mode),
            ..Default::default()
        },
    }
}

#[test]
fn listing_reuses_only_identical_bytes_even_with_unchanged_metadata() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Test.json");
    let mut listing = Listing::default();
    let old = profile("Test", 0);
    let new = profile("Test", 1);
    let original = serde_json::to_vec(&old).unwrap();
    let changed = serde_json::to_vec(&new).unwrap();
    assert_eq!(original.len(), changed.len());
    fs::write(&path, &original).unwrap();

    let first = listing.read(root.path()).unwrap().remove(0);
    let cached_bytes = listing.profiles[&path].bytes.as_ptr();
    let second = listing.read(root.path()).unwrap().remove(0);
    assert_eq!(listing.profiles[&path].bytes.as_ptr(), cached_bytes);
    assert_eq!(first.revision, second.revision);

    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    fs::write(&path, &changed).unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);

    let latest = listing.read(root.path()).unwrap().remove(0);
    assert_ne!(latest.revision, first.revision);
    assert_eq!(latest.revision, authorization::digest(&new).unwrap());
    assert!(check_revision(&path, "Test", Some(&first.revision)).is_err());
    assert_eq!(
        load_from(root.path(), "Test").unwrap().settings.fan_mode,
        Some(1)
    );
}

#[test]
fn listing_reports_corruption_and_wrong_names_then_evicts_removed_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Test.json");
    let renamed = root.path().join("Other.json");
    let bytes = serde_json::to_vec(&profile("Test", 0)).unwrap();
    let mut listing = Listing::default();
    fs::write(&path, &bytes).unwrap();
    assert_eq!(listing.read(root.path()).unwrap().len(), 1);

    fs::write(&path, b"{partial").unwrap();
    assert!(
        listing
            .read(root.path())
            .err()
            .unwrap()
            .to_string()
            .contains("损坏")
    );
    assert!(listing.profiles.is_empty());
    fs::write(&path, &bytes).unwrap();
    assert_eq!(listing.read(root.path()).unwrap().len(), 1);

    fs::rename(&path, &renamed).unwrap();
    assert!(
        listing
            .read(root.path())
            .err()
            .unwrap()
            .to_string()
            .contains("档案名与文件名不一致")
    );
    fs::rename(&renamed, &path).unwrap();
    assert_eq!(listing.read(root.path()).unwrap().len(), 1);
    fs::remove_file(&path).unwrap();
    assert!(listing.read(root.path()).unwrap().is_empty());
    assert!(listing.profiles.is_empty());
}

#[test]
fn listing_limits_retention_without_limiting_results() {
    let root = tempfile::tempdir().unwrap();
    let oversized = root.path().join("Large.json");
    let mut bytes = serde_json::to_vec(&profile("Large", 0)).unwrap();
    bytes.resize(MAX_CACHED_BYTES + 1, b' ');
    fs::write(&oversized, &bytes).unwrap();
    for index in 0..=MAX_CACHED_PROFILES {
        let name = format!("Profile {index:03}");
        let path = root.path().join(format!("{name}.json"));
        fs::write(path, serde_json::to_vec(&profile(&name, 0)).unwrap()).unwrap();
    }
    let mut listing = Listing::default();
    let result = listing.read(root.path()).unwrap();
    assert_eq!(result.len(), MAX_CACHED_PROFILES + 2);
    assert!(result.windows(2).all(|pair| pair[0].name < pair[1].name));
    assert_eq!(listing.profiles.len(), MAX_CACHED_PROFILES);
    assert!(!listing.profiles.contains_key(&oversized));
}

#[test]
fn concurrent_lists_follow_committed_profile_replacements() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path();
    save_in(dir, profile("Test", 0).settings, "Test", None).unwrap();
    let mut committed = vec![list_in(dir).unwrap().remove(0).revision];
    let barrier = Barrier::new(3);

    thread::scope(|scope| {
        let readers = [0, 1].map(|_| {
            scope.spawn(|| {
                barrier.wait();
                (0..16)
                    .map(|_| {
                        let mut entries = list_in(dir).unwrap();
                        assert_eq!(entries.len(), 1);
                        entries.remove(0).revision
                    })
                    .collect::<Vec<_>>()
            })
        });
        barrier.wait();
        for index in 0..16 {
            save_in(
                dir,
                profile("Test", index % 2).settings,
                "Test",
                committed.last().map(String::as_str),
            )
            .unwrap();
            committed.push(authorization::digest(&load_from(dir, "Test").unwrap()).unwrap());
        }
        for reader in readers {
            assert!(
                reader
                    .join()
                    .unwrap()
                    .iter()
                    .all(|revision| committed.contains(revision))
            );
        }
    });
    assert_eq!(
        &list_in(dir).unwrap()[0].revision,
        committed.last().unwrap()
    );
}
