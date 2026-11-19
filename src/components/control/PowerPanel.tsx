import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { SegmentedControl } from "../SegmentedControl";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { DiffPreview } from "./DiffPreview";
import { ScaleSlider } from "./ScaleSlider";
import { usePowerControl } from "./usePowerControl";
import { ParameterDescriptions } from "./ParameterHelp";
import { CPU_TUNING_HELP } from "./parameterDescriptions";

const TITLE = "CPU 功耗与电压";

export function PowerPanel({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const model = usePowerControl(active, revision, onSettled);
  const { profile, draft, dirty, busy, target } = model;
  if (!profile || !draft || !dirty) return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label={TITLE}>
      <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="power" />{TITLE}</h2>
        <span className="ccc-control-caption">{model.error ? "读取失败" : "读取中…"}</span>
        <div className="ccc-module-tools"><DetailsDialog title="CPU 参数与诊断" label="详情"><ParameterDescriptions items={CPU_TUNING_HELP} /></DetailsDialog></div>
      </div>
      {model.error && <><p role="alert" className="ccc-control-error">{model.error}</p>
        <Button icon="refresh" variant="ghost" size="compact" type="button" onClick={model.retry}>重试</Button></>}
    </Surface>
  );
  const unavailable = target === "runtime" && !profile.msrRuntimeAvailable;
  return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label={TITLE}>
      <div className="ccc-panel-head"><h2 className="ccc-panel-title"><Icon name="power" />{TITLE}</h2>
        <div className="ccc-module-tools"><DetailsDialog title="CPU 参数与诊断" label="详情">
          <ParameterDescriptions items={CPU_TUNING_HELP} />
          {[...profile.warnings, ...profile.runtimeWarnings, profile.msrRuntimeError].filter(Boolean)
            .map((warning, index) => <p key={index}>{warning}</p>)}
        </DetailsDialog></div>
      </div>
      <SegmentedControl label="应用方式" value={target} disabled={busy}
        options={[{ value: "runtime", label: "立即应用" }, { value: "saved", label: "保存设置" }]}
        onChange={value => model.setTarget(value as typeof target)} />
      {unavailable && <span className="ccc-control-caption is-warn">即时调节不可用</span>}
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
      {model.error && <p className="ccc-control-error" role="alert">{model.error}</p>}
      {model.reviews.map(({ scope, items, apply }) => <ConfigurationAction key={scope}
        label={scope === "runtime" ? "立即应用 · CPU" : "保存设置 · CPU"} revision={JSON.stringify([profile, items])}
        disabled={busy} onConfirm={apply}><DiffPreview items={items} /></ConfigurationAction>)}
      <ConfigurationAction label={TITLE} revision={JSON.stringify(profile)} restore disabled={busy || !profile.canRestore}
        onConfirm={model.restore}><p>恢复首次操作前的功耗上限与电压偏移。</p></ConfigurationAction>
    </Surface>
  );
}
