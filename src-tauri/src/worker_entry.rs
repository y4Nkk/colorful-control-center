//! 工作进程入口在窗口、控制租约及HAL装配之前分派。
use crate::{battery, display, gpu_power, platform, startup, watchdog};

pub(super) fn run(args: &[String]) -> Option<u32> {
    let flag = args.first()?.as_str();
    let result = match flag {
        "--configure-login" => return Some(startup::login::run_child(args)),
        "--remove-login" => {
            if !(1..=2).contains(&args.len()) {
                Err(anyhow::anyhow!("卸载参数无效"))
            } else {
                startup::login::remove(args.get(1).map(String::as_str))
            }
        }
        "--oem-lan-policy" => return Some(platform::lan::run_child(args)),
        "--oem-gpu-power" => return Some(gpu_power::run_child(args)),
        "--battery-calibration" => battery::calibration::run_child(),
        "--display-watchdog" => display::run_child(),
        "--fan-watchdog" => watchdog::run_child(),
        crate::shutdown::deadline::FLAG => {
            if args.len() == 1 {
                crate::shutdown::deadline::run_child()
            } else {
                Err(anyhow::anyhow!("退出监护启动参数无效"))
            }
        }
        _ => return None,
    };
    match result {
        Ok(()) => Some(0),
        Err(error) => {
            if flag == "--fan-watchdog" {
                watchdog::report_failure(&format!("{error:#}"));
            }
            eprintln!("[{flag}] {error:#}");
            Some(1)
        }
    }
}
