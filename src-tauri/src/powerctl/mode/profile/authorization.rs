use super::links;
use crate::{
    gpu_power::{mode as node, profile::Target as NodeTarget},
    gpuctl::turbo::mode as clocks,
    powerctl::mode::Target,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NodeRule {
    pub automatic_power_off: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub action: Option<node::Action>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ApprovalPreview {
    pub identity: String,
    pub target: Target,
    pub clocks: clocks::Cycle,
    pub node_rules: Vec<NodeRule>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub allowed_automatic_power_off: Option<Vec<bool>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Grant {
    pub identity: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub automatic_power_off: Option<Vec<bool>>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub driver: Option<String>,
    pub driver_deferred: bool,
}

pub(crate) fn authorization(
    context: &links::Context<'_>,
    plan: links::Plan,
    explicit: Option<&NodeTarget>,
    needs_gpu: bool,
) -> Result<ApprovalPreview> {
    let gpu = plan
        .target
        .gpu
        .as_ref()
        .context("档案模式GPU事实缺失")?
        .target()?;
    let wanted = plan.core.target.mode;
    let mut clocks = clocks::Cycle::new(gpu.clocks.policy.clone(), &[wanted]);
    context.inspect_cycle(
        &mut clocks,
        gpu.on_ac,
        gpu.node.as_ref().is_some_and(|node| !node.enabled),
    );
    if let Some(reason) = clocks.unavailable_reason() {
        bail!("档案未来供电GPU目标不可执行：{reason}");
    }
    if gpu
        .clocks
        .driver
        .as_ref()
        .zip(clocks.driver.as_ref())
        .is_some_and(|(before, after)| before != after)
    {
        bail!("档案GPU在授权预览期间已变化，请重新读取");
    }
    let node_rules = if gpu.node_supported {
        [false, true]
            .into_iter()
            .map(|automatic_power_off| {
                Ok(NodeRule {
                    automatic_power_off,
                    action: node::intent(wanted, None, false, automatic_power_off)?,
                })
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    let allowed_automatic_power_off = gpu.node.as_ref().map(|node| {
        let mut values = vec![node.automatic_power_off];
        if let Some(explicit) = explicit {
            values.push(explicit.values.automatic_power_off);
        }
        values.sort();
        values.dedup();
        values
    });
    future_node_preflight(
        gpu.node.as_ref(),
        &node_rules,
        allowed_automatic_power_off.as_deref(),
    )?;
    let identity = stable_identity(&plan, &clocks, &node_rules, explicit, needs_gpu)?;
    plan.core.review(context.hal)?;
    Ok(ApprovalPreview {
        identity,
        target: plan.target,
        clocks,
        node_rules,
        allowed_automatic_power_off,
    })
}

fn future_node_preflight(
    device: Option<&crate::gpu_power::Device>,
    rules: &[NodeRule],
    allowed: Option<&[bool]>,
) -> Result<()> {
    let Some(device) = device else {
        return Ok(());
    };
    for rule in rules
        .iter()
        .filter(|rule| allowed.is_some_and(|allowed| allowed.contains(&rule.automatic_power_off)))
    {
        if let Some(action) = rule.action {
            let future = device.projected(crate::gpu_power::Target {
                enabled: true,
                automatic_power_off: rule.automatic_power_off,
            });
            crate::gpu_power::operation::preflight(&future, future.target(), action.target)
                .context("档案未来独显联动不满足当前设备资格")?;
        }
    }
    Ok(())
}

fn stable_identity(
    plan: &links::Plan,
    clocks: &clocks::Cycle,
    rules: &[NodeRule],
    explicit: Option<&NodeTarget>,
    needs_gpu: bool,
) -> Result<String> {
    let gpu = plan
        .target
        .gpu
        .as_ref()
        .context("档案模式GPU事实缺失")?
        .target()?;
    let device = gpu.node.as_ref().map(|node| (&node.id, &node.name));
    let decisions: Vec<_> = clocks
        .choices
        .iter()
        .map(|choice| (choice.mode, choice.on_ac, &choice.candidate.decision))
        .collect();
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            "profile-mode-gpu-1",
            &plan.core.target,
            gpu.node_supported,
            device,
            rules,
            explicit,
            needs_gpu,
            &clocks.policy,
            decisions,
        ))?)
    ))
}

impl ApprovalPreview {
    pub(crate) fn grant(&self) -> Result<Grant> {
        let grant = Grant {
            identity: self.identity.clone(),
            automatic_power_off: self.allowed_automatic_power_off.clone(),
            driver: self
                .clocks
                .driver
                .as_ref()
                .map(|driver| driver.identity.clone()),
            driver_deferred: self.clocks.deferred_reason.is_some(),
        };
        grant.validate()?;
        Ok(grant)
    }
    pub(crate) fn check(&self, grant: &Grant) -> Result<()> {
        grant.validate()?;
        if self.identity != grant.identity {
            bail!("档案模式、设备或未来供电联动已变化，请重新预览并确认");
        }
        let node = self
            .target
            .gpu
            .as_ref()
            .context("档案模式GPU事实缺失")?
            .target()?
            .node
            .as_ref();
        match (node, &grant.automatic_power_off) {
            (None, None) => (),
            (Some(node), Some(allowed)) if allowed.contains(&node.automatic_power_off) => (),
            _ => bail!("档案当前自动关断偏好不属于已确认的范围"),
        }
        if let Some(driver) = &self.clocks.driver {
            grant.check_driver(&driver.identity)?;
        }
        Ok(())
    }
}
impl Grant {
    pub(crate) fn validate(&self) -> Result<()> {
        if !digest_valid(&self.identity)
            || self
                .driver
                .as_ref()
                .is_some_and(|driver| !digest_valid(driver))
        {
            bail!("档案模式授权摘要无效");
        }
        if self
            .automatic_power_off
            .as_ref()
            .is_some_and(|values| !matches!(values.as_slice(), [false] | [true] | [false, true]))
        {
            bail!("档案自动关断授权范围无效");
        }
        Ok(())
    }
    pub(crate) fn check_driver(&self, identity: &str) -> Result<()> {
        if !digest_valid(identity) {
            bail!("档案GPU驱动身份无效");
        }
        match self.driver.as_deref() {
            Some(expected) if expected == identity => Ok(()),
            None if self.driver_deferred => Ok(()),
            _ => bail!("档案GPU驱动能力已变化或未获得延后核对许可"),
        }
    }
    pub(crate) fn bind_driver(&mut self, identity: &str) -> Result<bool> {
        self.check_driver(identity)?;
        if self.driver.is_some() {
            return Ok(false);
        }
        self.driver = Some(identity.to_owned());
        Ok(true)
    }
}
fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
