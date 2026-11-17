//! 原厂供电事件的模式目标；只解析已读取事实，不访问设备。

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    Startup,
    SupplyChange,
    Resume,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts<'a> {
    pub product: u32,
    pub customer: u8,
    pub dc_mode_power: bool,
    pub quiet_plus_default: bool,
    pub ac_mode: u8,
    pub available: &'a [u8],
    pub on_ac: Option<bool>,
    pub trigger: Trigger,
}
pub fn target(facts: Facts<'_>) -> Result<Option<u8>, &'static str> {
    if facts.trigger == Trigger::SupplyChange && matches!(facts.product, 20720 | 20724) {
        return Ok(None);
    }
    if !facts.dc_mode_power && facts.customer != 22 {
        return Ok(None);
    }
    let on_ac = facts.on_ac.ok_or("供电状态未知")?;
    let target = if facts.dc_mode_power {
        if on_ac {
            facts.ac_mode
        } else if facts.trigger == Trigger::SupplyChange
            && facts.product == 16577
            && facts.customer == 7
        {
            3
        } else if facts.quiet_plus_default {
            4
        } else if facts.available.contains(&1) {
            1
        } else {
            0
        }
    } else if on_ac {
        2
    } else {
        3
    };
    if !facts.available.contains(&target) {
        return Err("原厂供电目标模式未被当前固件声明支持");
    }
    Ok(Some(target))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base<'a>() -> Facts<'a> {
        Facts {
            product: 20600,
            customer: 1,
            dc_mode_power: true,
            quiet_plus_default: false,
            ac_mode: 2,
            available: &[0, 1, 2, 3, 4],
            on_ac: Some(false),
            trigger: Trigger::SupplyChange,
        }
    }
    #[test]
    fn dc_chooses_original_priority_and_ac_reads_saved_choice() {
        assert_eq!(target(base()), Ok(Some(1)));
        assert_eq!(
            target(Facts {
                quiet_plus_default: true,
                ..base()
            }),
            Ok(Some(4))
        );
        assert_eq!(
            target(Facts {
                available: &[0, 2, 3],
                ..base()
            }),
            Ok(Some(0))
        );
        assert_eq!(
            target(Facts {
                on_ac: Some(true),
                ac_mode: 3,
                ..base()
            }),
            Ok(Some(3))
        );
    }
    #[test]
    fn special_product_customer_rule_is_only_for_supply_changes() {
        let special = Facts {
            product: 16577,
            customer: 7,
            ..base()
        };
        assert_eq!(target(special), Ok(Some(3)));
        assert_eq!(
            target(Facts {
                trigger: Trigger::Startup,
                ..special
            }),
            Ok(Some(1))
        );
        assert_eq!(
            target(Facts {
                trigger: Trigger::Resume,
                ..special
            }),
            Ok(Some(1))
        );
    }
    #[test]
    fn customer_22_and_excluded_amd_supply_events_remain_distinct() {
        let customer = Facts {
            dc_mode_power: false,
            customer: 22,
            ..base()
        };
        assert_eq!(target(customer), Ok(Some(3)));
        assert_eq!(
            target(Facts {
                on_ac: Some(true),
                ..customer
            }),
            Ok(Some(2))
        );
        assert_eq!(
            target(Facts {
                product: 20720,
                ..customer
            }),
            Ok(None)
        );
        assert_eq!(
            target(Facts {
                product: 20724,
                trigger: Trigger::Resume,
                ..customer
            }),
            Ok(Some(3))
        );
        assert_eq!(
            target(Facts {
                customer: 1,
                dc_mode_power: false,
                ..base()
            }),
            Ok(None)
        );
    }
    #[test]
    fn unknown_supply_or_unavailable_mode_never_guesses() {
        assert!(
            target(Facts {
                on_ac: None,
                ..base()
            })
            .is_err()
        );
        assert!(
            target(Facts {
                on_ac: Some(true),
                ac_mode: 5,
                ..base()
            })
            .is_err()
        );
        assert!(
            target(Facts {
                quiet_plus_default: true,
                available: &[0, 1, 2, 3],
                ..base()
            })
            .is_err()
        );
    }
}
