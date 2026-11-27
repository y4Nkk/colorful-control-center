import { useState } from "react";
import { applyGpuPower, readGpuPower } from "../../api/client";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { SegmentedControl } from "../SegmentedControl";
import { Select } from "../Select";
import { Surface } from "../Surface";
import { GpuPowerVisual } from "./GpuPowerVisual";
import { useDeviceSettings } from "./useDeviceSettings";

export function GpuPowerSettings({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const { profile, loaded, busy, error, run, reread } = useDeviceSettings(readGpuPower, active, revision);
  const [id, setId] = useState("");
  const devices = profile?.devices ?? [];
  const selected = devices.find(device => device.id === id) ?? devices[0];
  const value = selected && selected.enabled !== selected.automaticPowerOff ? String(selected.enabled) : "";
  const commit = (action: () => Promise<void>) => run(async () => {
    try { await action(); }
    finally { onSettled(); }
  }, "独显状态已回读确认");

  return <Surface as="section" variant="group" className="ccc-panel" aria-label="独显电源" aria-busy={busy || undefined}>
    <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="gpu" />独显电源</h2><div className="ccc-module-tools">
      {selected && <DetailsDialog title="设备与授权" label="详情">
        <p className="ccc-control-caption">{selected.name}</p><p>开启或关闭独显时，Windows 会弹出授权确认。关闭后，正在使用独显的游戏或程序可能中断。</p>
        {!selected.canDisable && <p className="ccc-control-caption">独显正在给显示器输出画面，或者无法确认关闭后屏幕是否还有其他输出，所以暂时不能关闭。</p>}
      </DetailsDialog>}
      <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取独显" disabled={busy} onClick={reread} />
    </div></div>
    {error && <p role="alert" className="ccc-control-error">{error}</p>}
    {selected ? <>
      <GpuPowerVisual label={busy ? "正在切换" : undefined} enabled={selected.enabled} automatic={selected.automaticPowerOff} showState={!value} />
      {devices.length > 1 && <Select label="独显电源控制器" value={selected.id} disabled={busy}
        options={devices.map(device => ({ value: device.id, label: device.name }))} onChange={setId} />}
      {!selected.canDisable && <p className="ccc-control-caption">关闭暂不可用 · 查看详情</p>}
      <SegmentedControl className="ccc-module-control-end" label="独显电源" value={value} disabled={busy}
        options={[{ value: "true", label: "启用独显" }, { value: "false", label: "关闭独显", disabled: !selected.canDisable }]}
        onChange={next => {
          if (next !== value && (next === "true" || selected.canDisable)) void commit(() => applyGpuPower(selected, next === "true"));
        }} />
    </> : loaded ? <GpuPowerVisual label={profile ? "未发现可用独显" : "当前状态"} enabled={null} automatic={null} />
      : <Loading label="正在读取独显电源…" />}
  </Surface>;
}
