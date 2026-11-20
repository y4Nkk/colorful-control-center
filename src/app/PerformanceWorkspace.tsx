import { useEffect, useRef } from "react";
import { FanPanel } from "../components/control/FanPanel";
import { FanDust } from "../components/control/FanDust";
import { PowerPanel } from "../components/control/PowerPanel";
import { OemPerformanceOptions } from "../components/control/OemPerformanceOptions";
import { OemPowerMode } from "../components/control/OemPowerMode";
import { GpuTuning } from "../components/performance/GpuTuning";
import type { HardwareWorkspaceProps } from "./useControlModel";

export function PerformanceWorkspace({ active, model, fanRequest }: HardwareWorkspaceProps & { fanRequest: number }) {
  const { revisions } = model;
  const workspace = useRef<HTMLDivElement>(null);
  const fanSettings = useRef<HTMLDivElement>(null);
  const handled = useRef(0);
  useEffect(() => {
    if (!active || handled.current === fanRequest) return;
    const content = workspace.current;
    const target = fanSettings.current;
    if (!content || !target) return;
    const locate = () => target.scrollIntoView({ block: "start", behavior: "instant" });
    const resize = new ResizeObserver(locate);
    const interaction = new AbortController();
    const stop = () => {
      resize.disconnect();
      interaction.abort();
    };
    const frame = requestAnimationFrame(() => {
      target.focus({ preventScroll: true });
      locate();
      // 上方模块异步回读会改变风扇设置的纵向位置。
      resize.observe(content);
      for (const event of ["pointerdown", "wheel", "keydown"]) {
        window.addEventListener(event, stop, { capture: true, signal: interaction.signal });
      }
      handled.current = fanRequest;
    });
    return () => {
      cancelAnimationFrame(frame);
      stop();
    };
  }, [active, fanRequest]);
  return <div className="ccc-control" ref={workspace}>
    <OemPowerMode active={active} revision={revisions.mode} onSettled={() => model.refreshOthers("mode")} />
    <OemPerformanceOptions active={active} revision={revisions.options} onSettled={() => model.refreshOthers("options")} />
    <div className="ccc-module-grid" data-module-layout="tuning">
      <PowerPanel active={active} revision={revisions.power} onSettled={() => model.refreshOthers("power")} />
      <GpuTuning active={active} revision={revisions.gpu} onSettled={() => model.refreshOthers("gpu")} />
    </div>
    <div ref={fanSettings} className="ccc-control" tabIndex={-1}>
      <FanPanel active={active} revision={revisions.fan} onSettled={() => model.refreshOthers("fan")}
        live={model.history.stale ? null : model.history.live} />
      <FanDust active={active} revision={revisions.fan + revisions.mode} onSettled={() => model.setRevisions(previous => ({
        ...previous, fan: previous.fan + 1, mode: previous.mode + 1, options: previous.options + 1,
      }))} />
    </div>
  </div>;
}
