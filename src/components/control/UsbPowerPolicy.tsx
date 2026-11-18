import { Icon } from "../Icon";
import { applyUsbPolicy, restoreUsbPolicy, type UsbPlan, type UsbTargets } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { useUsbPowerPolicy } from "./useUsbPowerPolicy";

import { USB_PLAN_NAMES as NAMES } from "./platform-values";
const text = (value: boolean | null) => value === null ? "未知" : value ? "开启" : "关闭";
const plans = (values: UsbPlan[], initial = false) => values.map(plan => ({ label: NAMES[plan.id],
  value: <div className="ccc-rule-facts">{(["ac", "dc"] as const).map(source => <span className="ccc-state-symbol" data-state={plan[source] === null ? "unknown" : plan[source] ? "on" : "off"} key={source}>
    <Icon name={source === "ac" ? "plug" : "battery"} size={16} />{source === "ac" ? "插电" : "电池"} · {initial && plan[source] === null ? "本次未修改" : text(plan[source])}
  </span>)}</div> }));
function changes(plans: UsbPlan[], draft: UsbTargets) {
  return plans.flatMap(plan => (["ac", "dc"] as const).flatMap(field => draft[field] === null || draft[field] === plan[field] ? [] : [{
    label: `${NAMES[plan.id]} · ${field === "ac" ? "插电" : "电池"}`, value: `${text(plan[field])} → ${text(draft[field])}`,
  }]));
}
const OPTIONS = [{ value: "keep", label: "保留各计划当前值" }, { value: "on", label: "开启选择性暂停" }, { value: "off", label: "关闭选择性暂停" }];
const selected = (value: boolean | null) => value === null ? "keep" : value ? "on" : "off";

export function UsbPowerPolicy({ active, revision }: { active: boolean; revision: number }) {
  const model = useUsbPowerPolicy(active, revision), { profile, draft, busy, error, run } = model;
  if (model.loaded && !profile && !error) return null;
  const preview = profile ? changes(profile.plans, draft) : [];
  const unknown = !!profile?.plans.some(plan => (draft.ac !== null && plan.ac === null) || (draft.dc !== null && plan.dc === null));
  const canApply = !!profile && profile.supported && !profile.blockedReason && preview.length > 0 && !unknown;
  const update = (field: keyof UsbTargets, value: string) => model.setDraft(previous => ({ ...previous, [field]: value === "keep" ? null : value === "on" }));
  return <Surface as="section" variant="group" className="ccc-panel ccc-usb-policy" aria-label="USB选择性暂停">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="usb" />USB 选择性暂停</h2>
      <div className="ccc-module-tools">{profile && <DetailsDialog title="USB 策略详情" label="详情">
        <FactList compact facts={plans(profile.plans)} />
        {profile.missing.length > 0 && <p>未安装 · {profile.missing.map(id => NAMES[id]).join("、")}</p>}
        {profile.restoreBlockedReason && <p>{profile.restoreBlockedReason}</p>}
        {profile.warnings.map(warning => <p key={warning}>{warning}</p>)}
      </DetailsDialog>}
      <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取USB策略" disabled={busy} onClick={model.reread} /></div></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "读取失败" : "读取中…"}</p> : <>
      <p className="ccc-control-caption">活动计划 · {profile.activePlan ? NAMES[profile.activePlan] : "自定义计划"}</p>
      <div className="ccc-display-controls">
        <label className="ccc-usb-field">插电时<Select label="插电USB策略" value={selected(draft.ac)} options={OPTIONS} disabled={busy} onChange={value => update("ac", value)} /></label>
        <label className="ccc-usb-field">电池供电时<Select label="电池USB策略" value={selected(draft.dc)} options={OPTIONS} disabled={busy} onChange={value => update("dc", value)} /></label>
        {profile.recommendation !== null && <Button className="ccc-inline-tool" variant="ghost" size="compact" disabled={busy || !profile.supported}
          onClick={() => model.setDraft({ ac: profile.recommendation, dc: profile.recommendation })}>使用原厂建议 · {text(profile.recommendation)}</Button>}
        {unknown && <p className="ccc-control-caption">所选字段含未知现值，无法保证恢复，请保留该字段或重新读取。</p>}
        {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
      </div>
      <ConfigurationAction label="USB 选择性暂停" revision={JSON.stringify([profile.revision, draft])} disabled={busy || !canApply}
        onConfirm={() => run(() => applyUsbPolicy(profile.revision, draft), "USB策略已回读，当前计划已重载")}>
        <FactList compact facts={preview} />
      </ConfigurationAction>
      <ConfigurationAction label="USB 选择性暂停" revision={profile.revision} restore disabled={busy || !profile.initial.length || !!profile.restoreBlockedReason}
        onConfirm={() => run(() => restoreUsbPolicy(profile.revision), "USB初始字段已恢复", true)}>
        <FactList compact facts={plans(profile.initial, true)} />
      </ConfigurationAction>
    </>}
  </Surface>;
}
