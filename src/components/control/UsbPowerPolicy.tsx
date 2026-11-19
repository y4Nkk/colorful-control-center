import { Icon } from "../Icon";
import { applyUsbPolicy, readUsbPolicy, restoreUsbPolicy, USB_POLICY_LABEL, type UsbPlan, type UsbTargets } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { useDeviceSettings } from "./useDeviceSettings";

import { USB_PLAN_NAMES as NAMES } from "./platform-values";
const text = (value: boolean | null) => value === null ? "未知" : value ? "开启" : "关闭";
const plans = (values: UsbPlan[], initial = false) => values.map(plan => ({ label: NAMES[plan.id],
  value: <div className="ccc-rule-facts">{(["ac", "dc"] as const).map(source => <span className="ccc-state-symbol" data-state={plan[source] === null ? "unknown" : plan[source] ? "on" : "off"} key={source}>
    <Icon name={source === "ac" ? "plug" : "battery"} size={16} />{source === "ac" ? "插电" : "电池"} · {initial && plan[source] === null ? "本次未修改" : text(plan[source])}
  </span>)}</div> }));
const OPTIONS = [{ value: "on", label: "开启选择性暂停" }, { value: "off", label: "关闭选择性暂停" }];

function sourceValue(plans: UsbPlan[], field: keyof UsbTargets) {
  const first = plans[0]?.[field];
  return first == null || !plans.every(plan => plan[field] === first) ? "" : first ? "on" : "off";
}

export function UsbPowerPolicy({ active, revision }: { active: boolean; revision: number }) {
  const { profile, loaded, busy, error, run, reread } = useDeviceSettings(readUsbPolicy, active, revision);
  if (loaded && !profile && !error) return null;
  const unknown = (field: keyof UsbTargets) => !profile?.plans.length || profile.plans.some(plan => plan[field] === null);
  const locked = busy || !profile?.supported || !!profile.blockedReason;
  const apply = (targets: UsbTargets) => {
    if (locked || !profile || (["ac", "dc"] as const).some(field => targets[field] !== null && unknown(field))) return;
    void run(() => applyUsbPolicy(profile.revision, targets), `${USB_POLICY_LABEL}已回读，当前计划已重载`);
  };
  const update = (field: keyof UsbTargets, value: string) => apply({ ac: null, dc: null, [field]: value === "on" });
  return <Surface as="section" variant="group" className="ccc-panel" aria-labelledby="usb-policy-heading">
    <div className="ccc-panel-head"><h2 id="usb-policy-heading" className="ccc-panel-title"><Icon name="usb" />{USB_POLICY_LABEL}</h2>
      <div className="ccc-module-tools">{profile && <DetailsDialog title={`${USB_POLICY_LABEL}详情`} label="详情">
        <FactList compact facts={plans(profile.plans)} />
        {profile.missing.length > 0 && <p>未安装 · {profile.missing.map(id => NAMES[id]).join("、")}</p>}
        {profile.restoreBlockedReason && <p>{profile.restoreBlockedReason}</p>}
        {profile.warnings.map(warning => <p key={warning}>{warning}</p>)}
      </DetailsDialog>}
      <Button icon="refresh" variant="ghost" size="icon-compact" aria-label={`重新读取 ${USB_POLICY_LABEL}`} disabled={busy} onClick={reread} /></div></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "读取失败" : "读取中…"}</p> : <>
      <p className="ccc-control-caption">活动计划 · {profile.activePlan ? NAMES[profile.activePlan] : "自定义计划"}</p>
      <div className="ccc-display-controls">
        {(["ac", "dc"] as const).map(field => <label key={field} className="ccc-usb-field">{field === "ac" ? "插电时" : "电池供电时"}
          <Select label={`${field === "ac" ? "插电时" : "电池供电时"}的 ${USB_POLICY_LABEL}`} value={sourceValue(profile.plans, field)} options={OPTIONS}
            placeholder={unknown(field) ? "当前值未知" : "各计划设置不同"} disabled={locked || unknown(field)} onChange={value => update(field, value)} />
        </label>)}
        {profile.recommendation !== null && <Button className="ccc-inline-tool" variant="ghost" size="compact" disabled={locked || unknown("ac") || unknown("dc")}
          onClick={() => apply({ ac: profile.recommendation, dc: profile.recommendation })}>使用原厂建议 · {text(profile.recommendation)}</Button>}
        {(unknown("ac") || unknown("dc")) && <p className="ccc-control-caption">请重新读取未知现值，确保改动可恢复。</p>}
        {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
      </div>
      <ConfigurationAction label={USB_POLICY_LABEL} revision={profile.revision} restore disabled={busy || !profile.initial.length || !!profile.restoreBlockedReason}
        onConfirm={() => run(() => restoreUsbPolicy(profile.revision), `${USB_POLICY_LABEL}已恢复初始设置`)}>
        <FactList compact facts={plans(profile.initial, true)} />
      </ConfigurationAction>
    </>}
  </Surface>;
}
