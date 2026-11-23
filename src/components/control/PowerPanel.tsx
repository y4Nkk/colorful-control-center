import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { Tabs } from "../Tabs";
import { ConfigurationAction } from "../ConfigurationAction";
import { DetailsDialog } from "../DetailsDialog";
import { DiffPreview } from "./DiffPreview";
import { ScaleSlider } from "./ScaleSlider";
import { CPU_PL_MAX, CPU_PL_MIN, usePowerControl } from "./usePowerControl";
import { ParameterDescriptions } from "./ParameterHelp";
import { CPU_TUNING_HELP } from "./parameterDescriptions";

const TITLE = "CPU 功耗与电压";

export function PowerPanel({ active, revision, onSettled }: { active: boolean; revision: number; onSettled: () => void }) {
  const model = usePowerControl(active, revision, onSettled);
  const { profile, draft, dirty, busy } = model;
  return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label={TITLE}>
      <div className="ccc-tuning-head">
        <h2 className="ccc-panel-title"><Icon name="power" />{TITLE}</h2>
        <div className="ccc-tuning-meta"><DetailsDialog title="CPU 参数与诊断" label="说明与诊断" icon={null}>
          <ParameterDescriptions items={CPU_TUNING_HELP} />
          {profile && [...profile.warnings, ...profile.runtimeWarnings, profile.msrRuntimeError].filter(Boolean)
            .map((warning, index) => <p key={index} className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{warning}</p>)}
        </DetailsDialog></div>
      </div>
      {profile && draft && dirty ? <CpuControls model={model} /> : <>
        <p className="ccc-control-caption">{model.error ? "读取失败" : "读取中…"}</p>
        {model.error && <div className="ccc-tuning-scope"><Button icon="refresh" variant="ghost" size="compact" onClick={model.retry}>重试</Button></div>}
      </>}
      {model.error && <p className="ccc-control-error" role="alert">{model.error}</p>}
      {profile && draft && dirty && model.reviews.map(({ scope, items, apply }) => <ConfigurationAction key={scope}
        label={scope === "runtime" ? "立即应用 · CPU" : "保存设置 · CPU"} revision={JSON.stringify([profile, items])}
        disabled={busy} onConfirm={apply}><DiffPreview items={items} /></ConfigurationAction>)}
      {profile && draft && dirty && <ConfigurationAction label={TITLE} revision={JSON.stringify(profile)} restore disabled={busy || !profile.canRestore}
        onConfirm={model.restore}><p>恢复首次操作前的功耗上限与电压偏移。</p></ConfigurationAction>}
    </Surface>
  );
}

function CpuControls({ model }: { model: ReturnType<typeof usePowerControl> }) {
  const { profile, draft, dirty, busy, target } = model;
  if (!profile || !draft || !dirty) return null;
  const unavailable = target === "runtime" && !profile.msrRuntimeAvailable;
  const disabled = busy || unavailable;
  const content = <div className="ccc-cpu-tab-content">
    {unavailable && <span className="ccc-control-caption is-warn">即时调节不可用</span>}
    <div className="ccc-tuning-columns">
      <section className="ccc-tuning-group" aria-label="功耗上限">
        <h3>功耗上限</h3>
        {!unavailable && !draft.limits && <p className="ccc-control-caption is-warn">缺少有效的成对功耗读数，暂不可调节功耗上限。</p>}
        <ScaleSlider label="持续功耗上限" unit="W" min={CPU_PL_MIN} max={CPU_PL_MAX} magnetInterval={5}
          current={dirty.current.limits?.pl1 ?? CPU_PL_MIN} draft={draft.limits?.pl1 ?? CPU_PL_MIN} unavailable={!draft.limits}
          disabled={disabled} onChange={(pl1) => model.change({ pl1 })} />
        <ScaleSlider label="短时功耗上限" unit="W" min={CPU_PL_MIN} max={CPU_PL_MAX} magnetInterval={5}
          current={dirty.current.limits?.pl2 ?? CPU_PL_MIN} draft={draft.limits?.pl2 ?? CPU_PL_MIN} unavailable={!draft.limits}
          disabled={disabled} onChange={(pl2) => model.change({ pl2 })} />
      </section>
      <section className="ccc-tuning-group" aria-label="电压偏移">
        <h3>电压偏移</h3>
        <ScaleSlider label="核心电压偏移" unit="mV" min={-150} max={50} magnetInterval={25}
          current={dirty.current.coreMv} draft={draft.coreMv}
          unavailable={!dirty.coreReady} disabled={disabled} onChange={(coreMv) => model.change({ coreMv })} />
        <ScaleSlider label="核显电压偏移" unit="mV" min={-150} max={0} magnetInterval={25}
          current={dirty.current.gtMv} draft={draft.gtMv}
          unavailable={!dirty.gtReady} disabled={disabled} onChange={(gtMv) => model.change({ gtMv })} />
      </section>
    </div>
  </div>;
  return <Tabs label="应用方式" value={target} onChange={model.setTarget} disabled={busy} tabs={[
    { value: "runtime", label: "立即应用", content },
    { value: "saved", label: "保存设置", content },
  ]} />;
}
