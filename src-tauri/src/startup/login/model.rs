use anyhow::{Result, bail};
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
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Tree {
    name: String,
    namespace: String,
    attributes: Vec<String>,
    text: String,
    children: Vec<Tree>,
}
fn tree(node: roxmltree::Node<'_, '_>) -> Tree {
    let mut attributes: Vec<_> = node
        .attributes()
        .map(|a| format!("{}={}", a.name(), a.value()))
        .collect();
    attributes.sort();
    let mut children: Vec<_> = node
        .children()
        .filter(|n| n.is_element())
        .map(tree)
        .collect();
    children.sort();
    Tree {
        name: node.tag_name().name().into(),
        namespace: node.tag_name().namespace().unwrap_or("").into(),
        attributes,
        text: if children.is_empty() {
            node.text().unwrap_or("").into()
        } else {
            String::new()
        },
        children,
    }
}
pub fn task_level(value: &str, executable: &str, user: &str) -> Result<Level> {
    if value.len() > 65536 {
        bail!("登录任务XML过大");
    }
    let document = roxmltree::Document::parse(value)?;
    let root = document.root_element();
    // 仅接受本程序固定任务；额外动作、触发器或用户均改变结构指纹。
    for level in [Level::Standard, Level::Administrator] {
        let expected = xml(executable, user, level).unwrap();
        let expected = roxmltree::Document::parse(&expected)?;
        if tree(root) == tree(expected.root_element())
            && root.descendants().filter(|n| n.is_element()).all(|n| {
                n.attributes().all(|a| {
                    matches!(
                        (n.tag_name().name(), a.name(), a.value()),
                        ("Task", "version", "1.2")
                            | ("Principal", "id", "Owner")
                            | ("Actions", "Context", "Owner")
                    )
                })
            })
        {
            return Ok(level);
        }
    }
    bail!("同名登录任务的程序、用户或执行设置不属于当前软件，未覆盖")
}
pub fn validate(state: &State, executable: &str, user: &str) -> Result<Level> {
    if state
        .run
        .as_ref()
        .is_some_and(|run| run != &format!("\"{executable}\" --autostart"))
    {
        bail!("同名旧自启项不属于当前程序，未覆盖");
    }
    state.task.as_ref().map_or(Ok(Level::Disabled), |task| {
        task_level(task, executable, user)
    })
}
