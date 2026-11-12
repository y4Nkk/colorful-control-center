//! 桌面 IPC 注册表：前端可调用命令的唯一清单。
//!
//! 契约：任何 `#[tauri::command]` 必须在此登记后才对前端可见；
//! 新命令的接入步骤见项目 AGENTS.md「全局化接入契约」。

macro_rules! handler {
    () => {
        tauri::generate_handler![
            crate::commands::app::get_backend_info,
            crate::commands::hardware::read_hardware_snapshot,
            crate::commands::fans::read_fan_profile,
            crate::commands::fans::apply_fan_curve,
            crate::commands::fans::set_fan_mode,
            crate::commands::fans::set_fan_offset,
            crate::commands::fans::restore_fan_default,
            crate::commands::power::read_power_profile,
            crate::commands::power::apply_pl_persist,
            crate::commands::power::apply_voltage_persist,
            crate::commands::power::apply_pl_runtime,
            crate::commands::power::apply_voltage_runtime,
            crate::commands::profiles::save_profile,
            crate::commands::profiles::list_profiles,
            crate::commands::profiles::load_profile,
            crate::commands::profiles::delete_profile,
            crate::commands::profiles::capture_profile_settings,
            crate::commands::profiles::apply_profile,
        ]
    };
}

pub(crate) use handler;
