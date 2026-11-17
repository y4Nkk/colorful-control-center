use super::*;

#[test]
fn all_notifications_describe_reports_without_querying_or_returning_device_controls() {
    let cases = [
        (Notice::CapsLock(false), "大写锁", "已关闭"),
        (Notice::CapsLock(true), "大写锁", "已开启"),
        (Notice::ScrollLock(false), "滚动锁", "已关闭"),
        (Notice::ScrollLock(true), "滚动锁", "已开启"),
        (Notice::Docking(false), "扩展坞", "已断开"),
        (Notice::Docking(true), "扩展坞", "已连接"),
        (Notice::Gps(false), "GPS", "已关闭"),
        (Notice::Gps(true), "GPS", "已开启"),
        (Notice::SilentMode(false), "静音模式", "已关闭"),
        (Notice::SilentMode(true), "静音模式", "已开启"),
        (Notice::MobileNetwork(false), "移动网络", "已关闭"),
        (Notice::MobileNetwork(true), "移动网络", "已开启"),
        (Notice::DustRunning, "风扇除尘", "正在运行"),
        (Notice::TurboFan, "Turbo 风扇", "已开启"),
        (Notice::Charger(true), "充电状态", "开始充电"),
        (Notice::Charger(false), "充电状态", "停止充电"),
    ];
    for (notice, title, state) in cases {
        let result = message(notice, IDENTITY, || {
            panic!("non-NumLock notices never enumerate keyboards")
        })
        .unwrap()
        .unwrap();
        assert_eq!(result.title, title);
        assert_eq!(result.detail, format!("原厂报告：{state}"));
        assert!(!result.error);
        assert!(result.level.is_none());
    }
}

#[test]
fn usb_keyboard_suppresses_both_numlock_reports_but_missing_inventory_is_an_error() {
    for enabled in [false, true] {
        assert!(
            message(Notice::NumLock(enabled), IDENTITY, || Ok(true))
                .unwrap()
                .is_none()
        );
        let result = message(Notice::NumLock(enabled), IDENTITY, || Ok(false))
            .unwrap()
            .unwrap();
        assert_eq!(result.title, "数字锁");
        assert!(
            result
                .detail
                .ends_with(if enabled { "已开启" } else { "已关闭" })
        );
        let error = message(Notice::NumLock(enabled), IDENTITY, || {
            anyhow::bail!("inventory unavailable")
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("inventory unavailable"));
        assert!(error.to_string().contains("USB键盘条件"));
    }
}

#[test]
fn stale_authorization_never_queries_devices_or_shows_notification() {
    assert!(
        message(
            Notice::Charger(true),
            "oem-state-notifications-1",
            || panic!("旧授权不能扩大报告范围")
        )
        .is_err()
    );
    assert!(
        message(Notice::NumLock(true), "other", || panic!(
            "must reject first"
        ))
        .is_err()
    );
}
