import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Select } from "../Select";
import { SegmentedControl } from "../SegmentedControl";
import { Button } from "../Button";
import { useCallback, useEffect, useRef, useState } from "react";
import { applyGpuPower, readGpuPower, restoreGpuPower, type GpuPowerProfile } from "../../api/client";
import { ConfirmButton } from "../ConfirmButton";
import { toast } from "../use-toast";
import { Collapsible } from "../Collapsible";
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
  return { profile, id, selected, draft: draft ?? (selected ? !selected.automaticPowerOff : true), setDraft, busy, error, run, reload,
    select: (value: string) => { setId(value); setDraft(null); } };
}
export function GpuPowerSettings({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const state = useGpuPower(active, revision, onSettled);
  const { profile, selected, draft, busy, error, run } = state;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="独显开关">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="gpu" />独显电源</h2><span className="ccc-control-caption">{busy ? "等待授权与原厂控制" : selected?.enabled ? "允许运行" : selected ? "已请求关断" : "未读取"}</span></div>
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    {selected ? <>
      <GpuPowerVisual label="电源控制" enabled={selected.enabled} automatic={selected.automaticPowerOff} />
      {(profile?.devices.length ?? 0) > 1 && <Select label="独显电源控制器" value={state.id} disabled={busy}
        options={profile?.devices.map(d => ({ value: d.id, label: d.name })) ?? []} onChange={state.select} />
      }
      <SegmentedControl label="独显目标状态" value={String(draft)} disabled={busy}
        options={[{ value: "true", label: "启用独显" }, { value: "false", label: "关闭独显", disabled: !selected.canDisable }]}
        onChange={value => state.setDraft(value === "true")} />
      <FactList compact layout="tiles" facts={[
        { label: "目标独显", value: draft ? "开启" : "关闭" },
        { label: "自动关断", value: `${selected.automaticPowerOff ? "允许" : "关闭"} → ${draft ? "关闭" : "允许"}` },
      ]} />
      {!draft && <p className="ccc-control-caption">关闭可能中断正在使用独显的程序。</p>}
      <Collapsible summary="设备与授权"><p>{selected.name}</p><p>确认后请求 Windows 授权，恢复保留首次操作前态。</p></Collapsible>
      {!selected.canDisable && <p className="ccc-control-caption">独显正在驱动显示器，或无法确认其他活动输出；关闭暂不可用。</p>}
      <div className="ccc-actions">
        <ConfirmButton key={JSON.stringify([selected, draft])} label="应用独显状态" confirmLabel={`确认${draft ? "启用" : "关闭"}此独显并请求系统授权？`} busy={busy} disabled={draft === selected.enabled && selected.automaticPowerOff === !draft || !draft && !selected.canDisable} onConfirm={() => run(() => applyGpuPower(selected, draft))} />
        <ConfirmButton icon="undo" variant="secondary" label="恢复独显初始" confirmLabel="恢复首次操作前的独显状态？" busy={busy} disabled={!profile?.canRestore} onConfirm={() => run(restoreGpuPower)} />
      </div>
    </> : <><GpuPowerVisual label="电源控制" enabled={null} automatic={null} /><p className="ccc-control-caption">{profile ? "未发现支持的独显电源接口。" : "独显状态未读取。"}</p><Button icon="refresh" variant="ghost" size="compact" disabled={busy} onClick={() => state.reload().catch(e => state.run(() => Promise.reject(e)))}>重新读取独显</Button></>}
  </Surface>;
}
