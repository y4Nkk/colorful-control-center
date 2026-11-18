import { Icon } from "../Icon";
import { applyLanPolicy, restoreLanPolicy, type LanField } from "../../api/client";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { DetailsDialog } from "../DetailsDialog";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { LAN_FIELDS, lanString, lanValue, lanMeaning } from "./lan-values";
import { useLanPolicy } from "./useLanPolicy";
const OPTIONS = [{ value: "keep", label: "保留当前节能设置" }, { value: "on", label: "开启网卡节能" }, { value: "off", label: "关闭网卡节能" }];

export function LanPolicy({ active, revision }: { active: boolean; revision: number }) {
  const model = useLanPolicy(active, revision), { profile, draft, busy, error, request, preview, run } = model;
  if (model.loaded && !profile && !error) return null;
  const device = profile?.frame.devices.find(value => value.identity.instance === draft.deviceId);
  const initial = profile?.initial.find(value => value.identity.instance === draft.deviceId);
  const sameIdentity = !!device && !!initial && JSON.stringify(device.identity) === JSON.stringify(initial.identity);
  const currentPower = lanString(device?.values.powerSavingMode), canEditPower = !!profile?.frame.capabilities.powerSaving && ["0", "1"].includes(currentPower ?? "");
  const canWake = !!profile?.frame.capabilities.wakePatch && !!device && ["s5WakeOnLan", "shutdownWake", "wakeOnSlot"].some(field => lanString(device.values[field as LanField]) !== null);
  const devices = profile?.frame.devices.map(value => ({ value: value.identity.instance, label: value.name })) ?? [];
  for (const value of profile?.initial ?? []) if (!devices.some(option => option.value === value.identity.instance)) devices.push({ value: value.identity.instance, label: "已移除网卡 · 保留恢复记录" });
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="网络驱动策略">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="network" />网络驱动策略</h2>
      <div className="ccc-module-tools">
        {profile && <DetailsDialog title="网卡设置详情" label="详情">
          {device && <FactList compact facts={[
            { label: "设备实例", value: device.identity.instance }, { label: "驱动绑定", value: device.identity.driver },
            ...(Object.keys(LAN_FIELDS) as LanField[]).map(field => ({ label: LAN_FIELDS[field],
              value: `当前 ${lanValue(device.values[field])} · 初始 ${lanValue(initial?.fields[field])}` })),
          ]} />}
          {preview.error && <p className="ccc-control-error">{preview.error}</p>}
          {profile.lastError && <p className="ccc-control-error">{profile.lastError}</p>}
        </DetailsDialog>}
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取网络策略" disabled={busy} onClick={model.reread} />
      </div>
    </div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile ? <p className="ccc-control-caption">{error ? "读取失败" : "读取中…"}</p> : <>
      <div className="ccc-display-controls">
        <label className="ccc-config-field">网卡<Select label="网络策略设备" value={draft.deviceId} options={devices} disabled={busy || !devices.length}
          onChange={deviceId => model.setDraft({ deviceId, powerSaving: null, applyWakePatch: false })} /></label>
        <label className="ccc-config-field">节能<Select label="网卡节能目标" value={draft.powerSaving === null ? "keep" : draft.powerSaving ? "on" : "off"}
          options={OPTIONS} disabled={busy || !canEditPower}
          onChange={value => model.setDraft(previous => ({ ...previous, powerSaving: value === "keep" ? null : value === "on" }))} /></label>
        <Checkbox label="应用原厂关机唤醒驱动补丁" checked={draft.applyWakePatch} disabled={busy || !canWake}
          onChange={event => model.setDraft(previous => ({ ...previous, applyWakePatch: event.target.checked }))} />
        {profile.frame.recommendation !== null && <Button className="ccc-inline-tool" variant="ghost" size="compact" disabled={busy || !canEditPower}
          onClick={() => model.setDraft(previous => ({ ...previous, powerSaving: profile.frame.recommendation }))}>使用原厂建议</Button>}
      </div>
      {initial && !sameIdentity && <p className="ccc-control-caption">网卡或驱动已更换，初始记录不可恢复。</p>}
      {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
      {preview.error && <p className="ccc-control-caption">无法预览这个目标 · 查看详情</p>}
      <ConfigurationAction label="网络驱动策略" revision={JSON.stringify([request, preview.changes])}
        disabled={busy || !request || !preview.ready || !preview.changes.length || !!profile.blockedReason || !!initial && !sameIdentity}
        onConfirm={() => run(() => applyLanPolicy(request!), "网卡驱动配置已回读")}>
        <p>{device?.name} · 确认后请求 Windows 授权。</p>
        <FactList compact facts={preview.changes.map(change => ({
          label: LAN_FIELDS[change.field], value: `${lanMeaning(change.field, change.before)} → ${lanMeaning(change.field, change.target)}`,
        }))} />
      </ConfigurationAction>
      <ConfigurationAction label="网络驱动策略" revision={JSON.stringify([profile.revision, draft.deviceId])} restore
        disabled={busy || !sameIdentity || !!profile.blockedReason}
        onConfirm={() => run(() => restoreLanPolicy(profile.revision, draft.deviceId), "网卡初始字段已回读", true)}>
        <p>{device?.name} · 恢复首次修改前的字段。</p>
        {initial && <FactList compact facts={Object.entries(initial.fields).map(([field, value]) =>
          ({ label: LAN_FIELDS[field as LanField], value: lanMeaning(field as LanField, value) }))} />}
      </ConfigurationAction>
    </>}
  </Surface>;
}
