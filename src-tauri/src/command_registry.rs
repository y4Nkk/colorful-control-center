//! 桌面 IPC 注册表：前端可调用命令的唯一清单。
//!
//! 契约：任何 `#[tauri::command]` 必须在此登记后才对前端可见；
//! 新命令的接入步骤见项目 AGENTS.md「全局化接入契约」。

macro_rules! handler {
    () => {
        crate::command_registry::restrict(tauri::generate_handler![
            crate::commands::airplane::read_airplane,
            crate::commands::airplane::apply_airplane,
            crate::commands::oem_owner::read_fn_key_close_target,
            crate::commands::oem_owner::close_fn_key,
            crate::commands::osd::read_osd,
            crate::commands::osd::preview_osd,
            crate::commands::hotkeys::read_hotkeys,
            crate::commands::hotkeys::restore_battery_alert_audio,
            crate::commands::hotkeys::read_hotkey_status,
            crate::commands::hotkeys::dismiss_battery_cold_warning,
            crate::commands::hotkeys::acknowledge_hotkey_navigation,
            crate::commands::hotkeys::apply_hotkeys,
            crate::commands::hotkeys::restore_hotkeys,
            crate::commands::hotkeys::reset_invalid_hotkeys,
            crate::commands::hotkeys::stop_hotkeys,
            crate::commands::hotkeys::restore_screen_brightness,
            crate::commands::hotkeys::restore_low_battery_brightness,
            crate::commands::hotkeys::end_panel_hotkey,
            crate::commands::hotkeys::restore_gpu_turbo,
            crate::commands::hotkeys::restore_battery_thresholds,
            crate::commands::hotkeys::restore_pd_event,
            crate::commands::microphone::read_microphones,
            crate::commands::microphone::apply_microphones,
            crate::commands::touchpad::read_touchpad,
            crate::commands::touchpad::apply_touchpad,
            crate::commands::app::get_backend_info,
            crate::commands::app::read_administrator_access,
            crate::commands::app::restart_application,
            crate::commands::app::restart_as_administrator,
            crate::commands::app::read_application_preparation,
            crate::commands::app::open_startup_logs,
            crate::commands::app::exit_application,
            crate::commands::app_update::check_application_update,
            crate::commands::app_update::download_application_update,
            crate::commands::app_update::cancel_application_update,
            crate::commands::preferences::get_app_preferences,
            crate::commands::preferences::set_app_preferences,
            crate::commands::preferences::list_system_fonts,
            crate::commands::preferences::set_shortcut_recording,
            crate::commands::window::window_minimize,
            crate::commands::window::window_toggle_maximize,
            crate::commands::window::window_is_maximized,
            crate::commands::window::window_close,
            crate::commands::hardware::update_hardware_monitoring,
            crate::commands::hardware::read_memory_capacity,
            crate::commands::hardware::read_system_inventory,
            crate::commands::hardware::read_gpu_details,
            crate::commands::battery::read_battery_profile,
            crate::commands::calibration::read_battery_calibration,
            crate::commands::calibration::start_battery_calibration,
            crate::commands::calibration::stop_battery_calibration,
            crate::commands::calibration::restore_battery_calibration,
            crate::commands::battery::apply_battery_charge,
            crate::commands::battery::read_energy_policy,
            crate::commands::battery::apply_energy_policy,
            crate::commands::battery::restore_energy_policy,
            crate::commands::lighting::read_perkey_lighting,
            crate::commands::lighting::read_lighting_options,
            crate::commands::lighting::read_multi_zone_lighting,
            crate::commands::lighting::apply_multi_zone_lighting,
            crate::commands::lighting::restore_multi_zone_lighting,
            crate::commands::lightbar::read_light_bar,
            crate::commands::lightbar::apply_light_bar,
            crate::commands::lightbar::restore_light_bar,
            crate::commands::lighting::apply_lighting_options,
            crate::commands::lighting::restore_lighting_options,
            crate::commands::lighting::apply_perkey_lighting,
            crate::commands::lighting::restore_perkey_lighting,
            crate::commands::keyboard::read_keyboard_settings,
            crate::commands::keyboard::apply_keyboard_settings,
            crate::commands::platform::read_platform_power,
            crate::commands::usb::read_usb_policy,
            crate::commands::usb::apply_usb_policy,
            crate::commands::usb::restore_usb_policy,
            crate::commands::lan::read_lan_policy,
            crate::commands::lan::preview_lan_policy,
            crate::commands::lan::apply_lan_policy,
            crate::commands::lan::restore_lan_policy,
            crate::commands::dust::read_fan_dust,
            crate::commands::keymap::read_keymap,
            crate::commands::keymap::preview_keymap_import,
            crate::commands::keymap::retry_keymap_programs,
            crate::commands::keymap::apply_keymap,
            crate::commands::keymap::restore_keymap,
            crate::commands::keymap::start_keymap_recording,
            crate::commands::keymap::read_keymap_recording,
            crate::commands::keymap::stop_keymap_recording,
            crate::commands::dust::start_fan_dust,
            crate::commands::dust::apply_fan_dust_schedule,
            crate::commands::dust::restore_fan_dust_schedule,
            crate::commands::platform::apply_pd_level,
            crate::commands::platform::restore_pd_level,
            crate::commands::devices::read_display_modes,
            crate::commands::devices::read_automatic_display_trial,
            crate::commands::devices::read_automatic_refresh,
            crate::commands::devices::enable_automatic_refresh,
            crate::commands::devices::restore_automatic_refresh,
            crate::commands::devices::apply_display_refresh,
            crate::commands::devices::apply_display_rotation,
            crate::commands::devices::keep_display_mode,
            crate::commands::devices::cancel_display_trial,
            crate::commands::devices::read_gpu_power,
            crate::commands::devices::read_graphics_mode,
            crate::commands::devices::apply_graphics_mode,
            crate::commands::devices::reboot_for_graphics_mode,
            crate::commands::devices::apply_gpu_power,
            crate::commands::fans::read_fan_profile,
        crate::commands::fans::save_custom_fan_curves,
            crate::commands::fans::set_fan_mode,
            crate::commands::fans::set_fan_offset,
            crate::commands::fans::restore_fan_settings,
            crate::commands::power::read_power_profile,
            crate::commands::power::read_windows_power_plans,
            crate::commands::power::apply_windows_power_plan,
            crate::commands::power::read_oem_power_mode,
            crate::commands::power::read_oem_performance_options,
            crate::commands::power::apply_oem_performance_options,
            crate::commands::power::apply_oem_power_mode,
            crate::commands::supply_mode::read_oem_supply_mode,
            crate::commands::supply_mode::enable_oem_supply_mode,
            crate::commands::supply_mode::resume_oem_supply_mode,
            crate::commands::supply_mode::pause_oem_supply_mode,
            crate::commands::supply_mode::restore_oem_supply_mode,
            crate::commands::supply_mode::archive_invalid_oem_supply_record,
            crate::commands::power::apply_saved_cpu,
            crate::commands::power::apply_runtime_cpu,
            crate::commands::power::restore_power_profile,
            crate::commands::gpu::read_gpu_profile,
            crate::commands::gpu::apply_gpu_clock_offsets,
            crate::commands::gpu::restore_gpu_baseline,
            crate::commands::lighting::read_keyboard_lighting,
            crate::commands::lighting::apply_keyboard_lighting,
            crate::commands::lighting::restore_keyboard_lighting,
            crate::commands::profiles::save_profile,
            crate::commands::profile_policy::read_power_profile_policy,
            crate::commands::profile_policy::save_power_profile_policy,
            crate::commands::profile_policy::set_power_profile_policy_enabled,
            crate::commands::profile_policy::pause_power_profile_policy,
            crate::commands::profile_policy::restore_power_profile_policy,
            crate::commands::profile_policy::reload_power_profile_policy,
            crate::commands::profile_policy::reset_power_profile_policy,
            crate::commands::profiles::list_profiles,
            crate::commands::profiles::read_builtin_profiles,
            crate::commands::profiles::preview_builtin_profile,
            crate::commands::profiles::apply_builtin_profile,
            crate::commands::profiles::load_profile,
            crate::commands::profiles::delete_profile,
            crate::commands::profiles::capture_profile_settings,
            crate::commands::profiles::preview_profile_application,
            crate::commands::profiles::preview_profile_authorization,
            crate::commands::profiles::apply_profile,
            crate::commands::profiles::preview_profile_display,
            crate::commands::profiles::preview_profile_gpu_power,
            crate::commands::profiles::preview_profile_graphics_mode,
            crate::commands::profiles::read_profile_application,
            crate::commands::profiles::keep_profile_display,
            crate::commands::profiles::cancel_profile_application,
            crate::commands::settings::get_autostart,
            crate::commands::settings::get_oem_startup_notice,
            crate::commands::settings::set_autostart,
            crate::commands::settings::get_startup_profile,
            crate::commands::settings::set_startup_profile,
            crate::commands::settings::export_profile,
            crate::commands::settings::preview_profile_import,
        ])
    };
}

pub(crate) use handler;

fn allowed(window: &str, command: &str) -> bool {
    window == "main" || window == "osd" && command == "read_osd"
}

fn allowed_during_preparation(command: &str) -> bool {
    matches!(
        command,
        "read_application_preparation"
            | "open_startup_logs"
            | "exit_application"
            | "restart_application"
            | "window_minimize"
            | "window_toggle_maximize"
            | "window_is_maximized"
            | "window_close"
    )
}

pub(crate) fn restrict(
    handler: impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    move |invoke| {
        use tauri::Manager;
        let webview = invoke.message.webview_ref();
        let command = invoke.message.command();
        if !allowed(webview.label(), command) {
            invoke.resolver.reject("此窗口只允许读取屏显状态");
            return true;
        }
        let Some(_operation) = crate::shutdown::enter() else {
            invoke.resolver.reject(crate::shutdown::CLOSING);
            return true;
        };
        if webview.label() == "main"
            && !allowed_during_preparation(command)
            && !crate::startup::ready(webview.app_handle())
        {
            invoke.resolver.reject("应用尚未完成初始化");
            return true;
        }
        handler(invoke)
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn preparation_only_admits_window_controls_and_recovery() {
        for command in [
            "read_application_preparation",
            "restart_application",
            "exit_application",
            "open_startup_logs",
            "window_close",
        ] {
            assert!(super::allowed_during_preparation(command));
        }
        for command in [
            "get_app_preferences",
            "set_app_preferences",
            "read_hotkey_status",
            "apply_runtime_cpu",
            "read_administrator_access",
            "restart_as_administrator",
            "update_hardware_monitoring",
        ] {
            assert!(!super::allowed_during_preparation(command));
        }
    }

    #[test]
    fn overlay_cannot_invoke_hardware_or_window_commands() {
        assert!(super::allowed("osd", "read_osd"));
        for command in [
            "preview_osd",
            "apply_runtime_cpu",
            "window_close",
            "read_administrator_access",
            "restart_as_administrator",
            "update_hardware_monitoring",
            "read_memory_capacity",
            "read_system_inventory",
            "read_gpu_details",
            "read_oem_supply_mode",
            "enable_oem_supply_mode",
            "resume_oem_supply_mode",
            "pause_oem_supply_mode",
            "restore_oem_supply_mode",
            "archive_invalid_oem_supply_record",
            "read_fn_key_close_target",
            "close_fn_key",
            "get_oem_startup_notice",
        ] {
            assert!(!super::allowed("osd", command));
            assert!(super::allowed("main", command));
        }
        assert!(!super::allowed("unknown", "read_osd"));
    }
}
