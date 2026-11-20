use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Disabled,
    Standard,
    Administrator,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    pub task: Option<String>,
    pub run: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub version: u8,
    pub before: State,
    pub applied: State,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub revision: String,
    pub level: Level,
    pub elevated: bool,
    pub can_restore: bool,
    pub legacy: bool,
}
pub fn revision(state: &State, record: &Option<Record>, executable_hash: &str) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(state, record, executable_hash))?)
    ))
}
pub fn xml(executable: &str, user: &str, level: Level) -> Option<String> {
    if level == Level::Disabled {
        return None;
    }
    let level = if level == Level::Administrator {
        "HighestAvailable"
    } else {
        "LeastPrivilege"
    };
    let executable = escape(executable);
    let user = escape(user);
    Some(format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
<RegistrationInfo><Author>ColorfulControlCenter</Author><Description>ColorfulControlCenter login startup v1</Description><URI>\ColorfulControlCenter-{user}</URI></RegistrationInfo>
<Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{user}</UserId></LogonTrigger></Triggers>
<Principals><Principal id="Owner"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>{level}</RunLevel></Principal></Principals>
<Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><AllowHardTerminate>false</AllowHardTerminate><StartWhenAvailable>true</StartWhenAvailable><RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable><IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings><AllowStartOnDemand>true</AllowStartOnDemand><Enabled>true</Enabled><Hidden>false</Hidden><RunOnlyIfIdle>false</RunOnlyIfIdle><WakeToRun>false</WakeToRun><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><Priority>7</Priority></Settings>
<Actions Context="Owner"><Exec><Command>{executable}</Command><Arguments>--autostart</Arguments></Exec></Actions></Task>"#
    ))
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
const NAMESPACE: &str = "http://schemas.microsoft.com/windows/2004/02/mit/task";
// 设置元素、唯一接受值、是否必须显式存在：任务服务存储时丢弃等于架构默认值的元素，
// 省略即默认值时才允许省略；必须项的架构默认值与模板不同，省略即语义偏离。
const SETTINGS: &[(&str, &str, bool)] = &[
    ("MultipleInstancesPolicy", "IgnoreNew", false),
    ("DisallowStartIfOnBatteries", "false", false),
    ("StopIfGoingOnBatteries", "false", false),
    ("AllowHardTerminate", "false", true),
    ("StartWhenAvailable", "true", true),
    ("ExecutionTimeLimit", "PT0S", true),
    ("Enabled", "true", false),
    ("Hidden", "false", false),
    ("AllowStartOnDemand", "true", false),
    ("RunOnlyIfIdle", "false", false),
    ("RunOnlyIfNetworkAvailable", "false", false),
    ("WakeToRun", "false", false),
    ("Priority", "7", false),
];
fn element<'a, 'i>(
    parent: roxmltree::Node<'a, 'i>,
    name: &str,
) -> Result<Option<roxmltree::Node<'a, 'i>>> {
    let mut found = None;
    for child in parent.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == name && found.replace(child).is_some() {
            bail!("登录任务包含多个{name}");
        }
    }
    Ok(found)
}
fn required<'a, 'i>(
    parent: roxmltree::Node<'a, 'i>,
    name: &str,
) -> Result<roxmltree::Node<'a, 'i>> {
    element(parent, name)?.ok_or_else(|| anyhow!("登录任务缺少{name}"))
}
fn only_child<'a, 'i>(
    parent: roxmltree::Node<'a, 'i>,
    name: &str,
) -> Result<roxmltree::Node<'a, 'i>> {
    let mut children = parent.children().filter(|n| n.is_element());
    let child = children.next().ok_or_else(|| anyhow!("登录任务缺少{name}"))?;
    if children.next().is_some() || child.tag_name().name() != name {
        bail!("登录任务{name}节不属于当前软件");
    }
    Ok(child)
}
fn text<'a>(node: roxmltree::Node<'a, '_>) -> &'a str {
    node.text().unwrap_or("").trim()
}
fn reject_other_children(node: roxmltree::Node<'_, '_>, allowed: &[&str]) -> Result<()> {
    for child in node.children().filter(|n| n.is_element()) {
        if !allowed.contains(&child.tag_name().name()) {
            bail!("登录任务包含未声明的{}节", child.tag_name().name());
        }
    }
    Ok(())
}
pub fn task_level(value: &str, executable: &str, user: &str, user_name: &str) -> Result<Level> {
    if value.len() > 65536 {
        bail!("登录任务XML过大");
    }
    let document = roxmltree::Document::parse(value)?;
    let root = document.root_element();
    if root.tag_name().name() != "Task"
        || root.tag_name().namespace() != Some(NAMESPACE)
        || root.attribute("version") != Some("1.2")
    {
        bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
    }
    // 注册信息由任务服务维护（补写日期、改写URI），不影响执行语义，不纳入指纹。
    reject_other_children(
        root,
        &["RegistrationInfo", "Triggers", "Principals", "Settings", "Actions"],
    )?;
    // 动作：唯一Exec，程序与参数逐字一致。
    let actions = required(root, "Actions")?;
    if actions.attribute("Context") != Some("Owner") {
        bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
    }
    let exec = only_child(actions, "Exec")?;
    if text(required(exec, "Command")?) != executable {
        bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
    }
    if text(required(exec, "Arguments")?) != "--autostart" {
        bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
    }
    reject_other_children(exec, &["Command", "Arguments"])?;
    // 触发器：唯一本用户登录触发；任务服务会把SID规范化成 机器\用户。
    let logon = only_child(required(root, "Triggers")?, "LogonTrigger")?;
    if let Some(enabled) = element(logon, "Enabled")? {
        if text(enabled) != "true" {
            bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
        }
    }
    let trigger_user = text(required(logon, "UserId")?);
    if trigger_user != user && trigger_user != user_name {
        bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
    }
    reject_other_children(logon, &["Enabled", "UserId"])?;
    // 主体：本用户、交互令牌；RunLevel省略即LeastPrivilege。
    let principal = only_child(required(root, "Principals")?, "Principal")?;
    if principal.attribute("id") != Some("Owner") {
        bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
    }
    let principal_user = text(required(principal, "UserId")?);
    if principal_user != user && principal_user != user_name {
        bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
    }
    if text(required(principal, "LogonType")?) != "InteractiveToken" {
        bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
    }
    let level = match element(principal, "RunLevel")?.map(text) {
        None | Some("LeastPrivilege") => Level::Standard,
        Some("HighestAvailable") => Level::Administrator,
        _ => bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖"),
    };
    reject_other_children(principal, &["UserId", "LogonType", "RunLevel"])?;
    // 设置：逐项等于模板值，或省略后架构默认值仍等于模板值。
    let settings = required(root, "Settings")?;
    let mut present = [false; SETTINGS.len()];
    let mut idle = None;
    for child in settings.children().filter(|n| n.is_element()) {
        let name = child.tag_name().name();
        if let Some(index) = SETTINGS.iter().position(|(n, _, _)| *n == name) {
            if text(child) != SETTINGS[index].1 {
                bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
            }
            present[index] = true;
        } else if name == "IdleSettings" && idle.replace(child).is_none() {
        } else {
            bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
        }
    }
    for (index, (name, _, explicit)) in SETTINGS.iter().enumerate() {
        if *explicit && !present[index] {
            bail!("登录任务缺少{name}设置，未覆盖");
        }
    }
    if let Some(idle) = idle {
        for child in idle.children().filter(|n| n.is_element()) {
            if !matches!(child.tag_name().name(), "StopOnIdleEnd" | "RestartOnIdle")
                || text(child) != "false"
            {
                bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖");
            }
        }
    }
    Ok(level)
}
pub fn validate(state: &State, executable: &str, user: &str, user_name: &str) -> Result<Level> {
    if state
        .run
        .as_ref()
        .is_some_and(|run| run != &format!("\"{executable}\" --autostart"))
    {
        bail!("同名旧自启项不属于当前程序，未覆盖");
    }
    state.task.as_ref().map_or(Ok(Level::Disabled), |task| {
        task_level(task, executable, user, user_name)
    })
}
