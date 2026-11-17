import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { SegmentedControl } from "../SegmentedControl";
import { ConfirmButton } from "../ConfirmButton";
import { Collapsible } from "../Collapsible";
import { DiffPreview } from "./DiffPreview";
import { ScaleSlider } from "./ScaleSlider";
import { usePowerControl } from "./usePowerControl";
import { ParameterHelp } from "./ParameterHelp";
import { CPU_TUNING_HELP } from "./parameterDescriptions";

export function PowerPanel({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const model = usePowerControl(active, revision, onSettled);
  const { profile, draft, dirty, busy, target } = model;
  if (!profile || !draft || !dirty) return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label="功耗与电压">
      <h2 className="ccc-panel-title"><Icon name="power" />功耗与电压</h2>
      <span className="ccc-chip">{model.error ? "读取失败" : "读取中…"}</span>
      <ParameterHelp title="CPU 调节参数说明" items={CPU_TUNING_HELP} />
      {model.error && <><p role="alert" className="ccc-control-error">{model.error}</p>
        <Button icon="refresh" variant="ghost" size="compact" type="button" onClick={model.retry}>重试</Button></>}
    </Surface>
  );
  const unavailable = target === "runtime" && !profile.msrRuntimeAvailable;
  return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label="功耗与电压">
      <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="power" />功耗与电压</h2></div>
      <SegmentedControl label="应用方式" value={target} disabled={busy}
        options={[{ value: "runtime", label: "立即应用" }, { value: "saved", label: "保存设置" }]}
        onChange={value => model.setTarget(value as typeof target)} />
      {unavailable && <span className="ccc-chip is-warn">即时调节不可用</span>}
      <ScaleSlider label="持续功耗上限" unit="W" min={35} max={157}
        current={dirty.current.pl1} draft={draft.pl1}
        unavailable={!dirty.plReady} disabled={busy || unavailable} onChange={(pl1) => model.change({ pl1 })} />
      <ScaleSlider label="短时功耗上限" unit="W" min={35} max={157}
        current={dirty.current.pl2} draft={draft.pl2}
        unavailable={!dirty.plReady} disabled={busy || unavailable} onChange={(pl2) => model.change({ pl2 })} />
      <ScaleSlider label="核心电压偏移" unit="mV" min={-150} max={50}
        current={dirty.current.coreMv} draft={draft.coreMv}
        unavailable={!dirty.coreReady} disabled={busy || unavailable} onChange={(coreMv) => model.change({ coreMv })} />
      <ScaleSlider label="核显电压偏移" unit="mV" min={-150} max={0}
        current={dirty.current.gtMv} draft={draft.gtMv}
        unavailable={!dirty.gtReady} disabled={busy || unavailable} onChange={(gtMv) => model.change({ gtMv })} />
      <DiffPreview items={dirty.items} />
      {model.error && <p className="ccc-control-error" role="alert">{model.error}</p>}
      <div className="ccc-actions">
        <ConfirmButton label={target === "runtime" ? "应用" : "保存"}
          confirmLabel={`确认${target === "runtime" ? "应用" : "保存"} ${dirty.items.map((item) => item.to).join(" / ")}？`}
          disabled={dirty.items.length === 0 || unavailable} busy={busy} onConfirm={model.apply} />
        <ConfirmButton icon="undo" variant="secondary" label="恢复初始" confirmLabel="恢复功耗与电压？" disabled={!profile.canRestore}
          busy={busy} onConfirm={model.restore} />
      </div>
      <ParameterHelp title="CPU 调节参数说明" items={CPU_TUNING_HELP} />
      <Collapsible summary="诊断">
        {[...profile.warnings, ...profile.runtimeWarnings, profile.msrRuntimeError].filter(Boolean)
          .map((warning, index) => <p key={index}>{warning}</p>)}
      </Collapsible>
    </Surface>
  );
}
