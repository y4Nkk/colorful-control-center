use super::*;

fn metadata(label: &str) -> Result<(String, String)> {
    Ok((label.into(), "NTFS".into()))
}

#[test]
fn reuses_verified_metadata_until_expiry_then_reads_the_current_label() {
    let mut cache = MetadataCache::new();
    let now = Instant::now();
    let guid = Some([1; 50]);
    assert_eq!(
        cache
            .metadata(b'C', guid, now, || metadata("原卷标"))
            .unwrap()
            .0,
        "原卷标"
    );
    let next = now + METADATA_MAX_AGE - Duration::from_secs(1);
    assert_eq!(
        cache
            .metadata(b'C', guid, next, || panic!("元数据有效期内不重读"))
            .unwrap()
            .0,
        "原卷标"
    );
    assert_eq!(
        cache
            .metadata(b'C', guid, now + METADATA_MAX_AGE, || metadata("新卷标"))
            .unwrap()
            .0,
        "新卷标"
    );
}

#[test]
fn replacement_invalidates_drive_letter_and_failed_refresh_clears_old_metadata() {
    let mut cache = MetadataCache::new();
    let now = Instant::now();
    cache
        .metadata(b'C', Some([1; 50]), now, || metadata("原卷"))
        .unwrap();
    let replacement = Some([2; 50]);
    assert!(
        cache
            .metadata(b'C', replacement, now, || Err(Error::other("隔离错误")))
            .is_err()
    );
    assert!(cache.0.is_empty());
    assert_eq!(
        cache
            .metadata(b'C', replacement, now, || metadata("新卷"))
            .unwrap()
            .0,
        "新卷"
    );
    assert_eq!(
        cache
            .metadata(b'C', None, now, || metadata("未验证标识"))
            .unwrap()
            .0,
        "未验证标识"
    );
    assert!(cache.0.is_empty());
}
