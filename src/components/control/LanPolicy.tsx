import { Icon } from "../Icon";
import { applyLanPolicy, previewLanPolicy, readLanPolicy, restoreLanPolicy, type LanField, type LanRequest } from "../../api/client";
import { useState } from "react";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { ConfigurationAction } from "../ConfigurationAction";
import { FactList } from "../FactList";
import { OemConflictAction } from "../OemConflictAction";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { LAN_FIELDS, lanString, lanValue, lanMeaning } from "./lan-values";
import { useDeviceSettings } from "./useDeviceSettings";
import { useLanWakePreview } from "./useLanWakePreview";
const OPTIONS = [{ value: "on", label: "开启网卡节能" }, { value: "off", label: "关闭网卡节能" }];

export function LanPolicy({ active, revision }: { active: boolean; revision: number }) {
  const { profile, loaded, busy, error, run, reread } = useDeviceSettings(readLanPolicy, active, revision);
  const [selected, setSelected] = useState("");
  const [reviewing, setReviewing] = useState(false);
  const devices = profile?.frame.devices.map(value => ({ value: value.identity.instance, label: value.name })) ?? [];
  for (const value of profile?.initial ?? []) if (!devices.some(option => option.value === value.identity.instance)) devices.push({ value: value.identity.instance, label: "已移除网卡 · 保留恢复记录" });
  const deviceId = devices.some(option => option.value === selected) ? selected : devices[0]?.value ?? "";
  const device = profile?.frame.devices.find(value => value.identity.instance === deviceId);
  const initial = profile?.initial.find(value => value.identity.instance === deviceId);
  const sameIdentity = !!device && !!initial && JSON.stringify(device.identity) === JSON.stringify(initial.identity);
  const currentPower = lanString(device?.values.powerSavingMode);
  const locked = busy || !!profile?.blockedReason || !!initial && !sameIdentity;
  const canEditPower = !!profile?.frame.capabilities.powerSaving && ["0", "1"].includes(currentPower ?? "");
  const canWake = !!profile?.frame.capabilities.wakePatch && !!device
    && (["s5WakeOnLan", "shutdownWake", "wakeOnSlot"] as const).some(field => lanString(device.values[field]) !== null);
  const request: LanRequest | null = profile && reviewing && canWake && !locked
    ? { expectedRevision: profile.revision, deviceId, powerSaving: null, applyWakePatch: true } : null;
  const preview = useLanWakePreview(active && !busy, request);
  if (loaded && !profile && !error) return null;
  const changePower = (enabled: boolean) => {
    if (!profile || locked || !canEditPower || (currentPower === "1") === enabled) return;
    const target = { expectedRevision: profile.revision, deviceId, powerSaving: enabled, applyWakePatch: false };
    void run(async () => {
      if ((await previewLanPolicy(target)).length) await applyLanPolicy(target);
    }, "网卡驱动配置已回读");
  };
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
        <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取网络策略" disabled={busy} onClick={reread} />
      </div>
    </div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    <OemConflictAction issues={[error, profile?.blockedReason, profile?.lastError, preview.error]} disabled={busy} onClosed={reread} />
    {!profile ? <p className="ccc-control-caption">{error ? "读取失败" : "读取中…"}</p> : <>
      <div className="ccc-display-controls">
        <label className="ccc-config-field">网卡<Select label="网络策略设备" value={deviceId} options={devices} disabled={busy || !devices.length}
          onChange={id => { setSelected(id); setReviewing(false); }} /></label>
        <label className="ccc-config-field">节能<Select label="网卡节能" value={canEditPower ? currentPower === "1" ? "on" : "off" : ""}
          options={OPTIONS} disabled={locked || !canEditPower} placeholder="设置未知" onChange={value => changePower(value === "on")} /></label>
        {profile.frame.recommendation !== null && <Button className="ccc-inline-tool" variant="ghost" size="compact" disabled={locked || !canEditPower}
          onClick={() => changePower(profile.frame.recommendation!)}>使用原厂建议</Button>}
        {canWake && <Button className="ccc-inline-tool" variant="secondary" size="compact" disabled={locked} aria-expanded={reviewing}
          onClick={() => setReviewing(value => !value)}>关机唤醒补丁</Button>}
      </div>
      {initial && !sameIdentity && <p className="ccc-control-caption">网卡或驱动已更换，初始记录不可恢复。</p>}
      {profile.blockedReason && <p className="ccc-control-caption">{profile.blockedReason}</p>}
      {preview.error && <p className="ccc-control-error" role="alert">{preview.error}</p>}
      {request && preview.ready && preview.changes.length === 0 && <p className="ccc-control-caption">当前驱动无需应用补丁。</p>}
      {request && <ConfigurationAction label="关机唤醒补丁" applyLabel="应用关机唤醒补丁" revision={JSON.stringify(request)}
        disabled={!preview.ready || !preview.changes.length}
        onConfirm={() => run(() => applyLanPolicy(request), "网卡驱动配置已回读")}>
        <p>{device?.name} · 需要 Windows 授权。</p>
        <FactList compact facts={preview.changes.map(change => ({
          label: LAN_FIELDS[change.field], value: `${lanMeaning(change.field, change.before)} → ${lanMeaning(change.field, change.target)}`,
        }))} />
      </ConfigurationAction>}
      <ConfigurationAction label="网络驱动策略" revision={JSON.stringify([profile.revision, deviceId])} restore
        disabled={busy || !sameIdentity || !!profile.blockedReason}
        onConfirm={() => run(() => restoreLanPolicy(profile.revision, deviceId), "网卡初始字段已回读")}>
        <p>{device?.name} · 恢复首次修改前的字段。</p>
        {initial && <FactList compact facts={Object.entries(initial.fields).map(([field, value]) =>
          ({ label: LAN_FIELDS[field as LanField], value: lanMeaning(field as LanField, value) }))} />}
      </ConfigurationAction>
    </>}
  </Surface>;
}
