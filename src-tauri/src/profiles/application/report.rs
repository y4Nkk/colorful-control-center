pub(crate) fn describe_application(result: &crate::profiles::Application) -> String {
    let name = &result.name;
    if let Some(error) = result
        .pending
        .as_ref()
        .and_then(|pending| pending.error.as_ref())
    {
        return format!("{name} 显示恢复待处理：{error}；请查看执行结果");
    }
    if result.pending.is_some() {
        return format!("{name} 等待显示确认或恢复；请查看档案执行结果");
    }
    let failures: Vec<_> = result
        .steps
        .iter()
        .filter(|s| !s.ok)
        .map(|s| format!("{}：{}", s.step, s.message))
        .collect();
    if result.targets_applied && failures.is_empty() {
        format!("已完成 {name} 的全部步骤")
    } else if result.targets_applied {
        format!("{name} 目标设置已应用，收尾需处理：{}", failures.join("；"))
    } else {
        format!("{name} 应用未完成：{}", failures.join("；"))
    }
}

pub(crate) fn log_application(result: &crate::profiles::Application) {
    if result.origin == super::Origin::Manual {
        return;
    }
    let message = describe_application(result);
    if result.origin == super::Origin::Startup {
        log::info!("[startup] {message}");
    } else if result.origin.automatic() {
        log::info!("[power-policy:{:?}] {message}", result.origin);
    }
}
