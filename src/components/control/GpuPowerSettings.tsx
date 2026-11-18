import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Select } from "../Select";
import { SegmentedControl } from "../SegmentedControl";
import { Button } from "../Button";
import { useCallback, useEffect, useRef, useState } from "react";
import { applyGpuPower, readGpuPower, restoreGpuPower, type GpuPowerProfile } from "../../api/client";
import { ConfigurationAction } from "../ConfigurationAction";
import { toast } from "../use-toast";
import { DetailsDialog } from "../DetailsDialog";
import { FactList } from "../FactList";
import { GpuPowerVisual } from "./GpuPowerVisual";

function useGpuPower(active: boolean, revision: number, onSettled: () => void) {
  const [profile, setProfile] = useState<GpuPowerProfile | null>(null);
  const [id, setId] = useState(""), [draft, setDraft] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const running = useRef(false), generation = useRef(0);
  const reload = useCallback(async () => {
    const read = ++generation.current;
    let next: GpuPowerProfile;
    try { next = await readGpuPower(); } catch (e) { if (read === generation.current) { setProfile(null); throw e; } else return; }
    if (read !== generation.current) return;
    setProfile(next); setId(previous => next.devices.some(d => d.id === previous) ? previous : next.devices[0]?.id ?? "");
  }, []);
  useEffect(() => { if (active) void reload().catch(e => setError(String(e))); return () => { ++generation.current; }; }, [active, revision, reload]);
  const run = async (action: () => Promise<void>) => {
    if (running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try { await action(); await reload(); setDraft(null); toast({ kind: "success", title: "独显状态已回读确认" }); }
    catch (e) { setError(String(e)); await reload().catch(readError => setError(previous => `${previous}；回读失败：${readError}`)); }
    finally { running.current = false; setBusy(false); onSettled(); }
  };
  const selected = profile?.devices.find(d => d.id === id);
  return { profile, id, selected, draft: draft ?? (selected ? !selected.automaticPowerOff : true), edited: draft !== null, setDraft, busy, error, run,
    reread: () => reload().then(() => setError(null)).catch(e => setError(String(e))),
    select: (value: string) => { setId(value); setDraft(null); } };
}
export function GpuPowerSettings({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const state = useGpuPower(active, revision, onSettled);
  const { profile, selected, draft, busy, error, run } = state;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="独显开关">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="gpu" />独显电源</h2><div className="ccc-module-tools">
      {selected && <DetailsDialog title="设备与授权" label="详情">
        <p>{selected.name}</p><p>确认后请求 Windows 授权；恢复首次操作前的独显状态。</p>
        {!selected.canDisable && <p>独显正在驱动显示器，或无法确认其他活动输出，暂不能关闭。</p>}
      </DetailsDialog>}
      <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取独显" disabled={busy} onClick={state.reread} />
    </div></div>
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    {selected ? <>
      <GpuPowerVisual label={busy ? "等待授权" : "当前状态"} enabled={selected.enabled} automatic={selected.automaticPowerOff} />
      {(profile?.devices.length ?? 0) > 1 && <Select label="独显电源控制器" value={state.id} disabled={busy}
        options={profile?.devices.map(d => ({ value: d.id, label: d.name })) ?? []} onChange={state.select} />
      }
      {!selected.canDisable && <p className="ccc-control-caption">关闭暂不可用 · 查看详情</p>}
      <SegmentedControl className="ccc-module-control-end" label="独显目标状态" value={state.edited ? String(draft) : "keep"} disabled={busy}
        options={[{ value: "keep", label: "保留当前" }, { value: "true", label: "启用独显" },
          { value: "false", label: "关闭独显", disabled: !selected.canDisable }]}
        onChange={value => state.setDraft(value === "keep" ? null : value === "true")} />
      <ConfigurationAction revision={JSON.stringify([selected, draft])} label="独显电源"
        disabled={busy || !state.edited || draft === selected.enabled && selected.automaticPowerOff === !draft || !draft && !selected.canDisable}
        onConfirm={() => run(() => applyGpuPower(selected, draft))}>
        <p>{selected.name}</p>
        <FactList compact facts={[
          { label: "独显电源", value: `${selected.enabled ? "开启" : "关闭"} → ${draft ? "开启" : "关闭"}` },
          { label: "自动关断", value: `${selected.automaticPowerOff ? "允许" : "关闭"} → ${draft ? "关闭" : "允许"}` },
        ]} />
        <p>确认后请求 Windows 授权。{!draft && "关闭可能中断正在使用独显的程序。"}</p>
      </ConfigurationAction>
    </> : <GpuPowerVisual label={profile ? "未发现可用独显" : "当前状态"} enabled={null} automatic={null} />}
    <ConfigurationAction revision={JSON.stringify(profile?.devices)} label="独显电源" restore disabled={busy || !profile?.canRestore}
      onConfirm={() => run(restoreGpuPower)}>
      <p>恢复首次操作前的独显电源与自动关断状态，随后回读确认。</p>
    </ConfigurationAction>
  </Surface>;
}
