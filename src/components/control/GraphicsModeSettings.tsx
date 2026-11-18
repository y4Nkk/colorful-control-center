import { Icon } from "../Icon";
import { useCallback, useEffect, useRef, useState } from "react";
import { applyGraphicsMode, readGraphicsMode, rebootForGraphicsMode, restoreGraphicsMode, type GraphicsModeProfile, type GraphicsModeVerification } from "../../api/client";
import { Button } from "../Button";
import { ConfirmButton } from "../ConfirmButton";
import { ConfigurationAction } from "../ConfigurationAction";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { toast } from "../use-toast";

const MODES = [{ value: 1, label: "仅集显" }, { value: 2, label: "仅独显" }, { value: 3, label: "混合显卡" }, { value: 4, label: "动态切换" }];
const name = (mode: number) => MODES.find(item => item.value === mode)?.label ?? "未知";
const STATUS: Record<GraphicsModeVerification, string> = { uncertain: "上次操作未完成，请重新发送或恢复", pendingReboot: "已发送选择，重启后确认", confirmed: "重启后已确认", notApplied: "重启后未匹配目标，可重新发送或恢复" };

function useGraphicsMode(active: boolean, revision: number) {
  const [profile, setProfile] = useState<GraphicsModeProfile | null>(null), [target, setTarget] = useState<number | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const running = useRef(false), generation = useRef(0);
  const reload = useCallback(async (reset: boolean) => {
    const id = ++generation.current;
    let next: GraphicsModeProfile;
    try { next = await readGraphicsMode(); } catch (e) { if (id === generation.current) { setProfile(null); throw e; } else return; }
    if (id !== generation.current) return;
    setProfile(next); if (reset) setTarget(null);
  }, []);
  useEffect(() => { if (active) void reload(false).catch(e => setError(String(e))); return () => { ++generation.current; }; }, [active, revision, reload]);
  const run = async (action: () => Promise<void>, message: string) => {
    if (running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try { await action(); await reload(true); toast({ kind: "success", title: message }); }
    catch (e) { setError(String(e)); await reload(false).catch(readError => setError(previous => `${previous}；回读失败：${readError}`)); }
    finally { running.current = false; setBusy(false); }
  };
  return { profile, draft: target ?? profile?.current?.mode ?? 0, edited: target !== null, setTarget, busy, error, run,
    reread: () => reload(false).then(() => setError(null)).catch(e => setError(String(e))) };
}
export function GraphicsModeSettings({ active, revision }: { active: boolean; revision: number }) {
  const { profile, draft, edited, setTarget, busy, error, run, reread } = useGraphicsMode(active, revision);
  const current = profile?.current, last = profile?.lastRequest;
  const choices = MODES.filter(item => current && (current.availableMask & (1 << (item.value - 1))) !== 0);
  const unchanged = !last && current?.mode === draft || last?.verification === "confirmed" && last.target === draft;
  return <Surface as="section" variant="group" className="ccc-panel" aria-label="显卡模式">
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="gpu" />显卡模式</h2>
      <div className="ccc-module-tools"><Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取显卡模式" disabled={busy} onClick={reread} /></div></div>
    {error && <p className="ccc-control-error" role="alert">{error}</p>}
    {!profile || !current ? <p className="ccc-control-caption">{profile ? "本机未提供已确认的模式切换接口。" : "正在读取显卡模式。"}</p> : <>
      <div className="ccc-module-summary"><strong>{name(current.mode)}</strong>
        <span className="ccc-control-caption" role="status">{last ? STATUS[last.verification] : "重启后生效"}</span></div>
      {last && (last.verification === "uncertain" || last.verification === "notApplied") && choices.some(choice => choice.value === last.target) &&
        <Button className="ccc-inline-tool" icon="refresh" variant="ghost" size="compact" disabled={busy}
          onClick={() => setTarget(last.target)}>重试上次选择</Button>}
      <label className="ccc-config-field ccc-module-control-end">选择模式
        <Select label="目标显卡模式" value={String(draft)} disabled={busy} options={choices.map(item => ({ value: String(item.value), label: item.label }))} onChange={value => setTarget(Number(value))} /></label>
      {last && (current.availableMask & (1 << (last.initialMode - 1))) === 0 && <p className="ccc-control-caption">当前固件未提供原模式，无法发送恢复选择。</p>}
      <ConfigurationAction revision={JSON.stringify([profile.revision, draft])} label="显卡模式"
        disabled={busy || !edited || unchanged || !choices.some(item => item.value === draft)} onConfirm={() => run(() => applyGraphicsMode(profile.revision, draft), "显卡选择已发送，重启后确认")}>
        <p>{name(current.mode)} → {name(draft)}</p><p className="ccc-control-caption">保存选择后，重启 Windows 生效。</p>
      </ConfigurationAction>
      <ConfigurationAction revision={profile.revision} label="显卡模式" restore disabled={busy || !profile.canRestore}
        onConfirm={() => run(() => restoreGraphicsMode(profile.revision), "原模式选择已发送，重启后确认")}>
        <p>恢复{last ? name(last.initialMode) : "初始模式"}，重启后生效。</p>
      </ConfigurationAction>
      {last?.verification === "pendingReboot" && <div className="ccc-actions"><ConfirmButton key={`reboot-${profile.revision}`}
        label="重启 Windows" confirmLabel="已保存工作，确认重启 Windows" busy={busy}
        onConfirm={() => run(() => rebootForGraphicsMode(profile.revision), "Windows 重启请求已发送")} /></div>}
    </>}
  </Surface>;
}
