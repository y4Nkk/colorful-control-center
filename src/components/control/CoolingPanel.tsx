import { useCallback, useId, useRef, useState } from "react";
import { Icon } from "../Icon";
import { Loading } from "../Loading";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { DetailsDialog } from "../DetailsDialog";
import { OemConflictAction } from "../OemConflictAction";
import { CoolingStrategy } from "./CoolingStrategy";
import { FanWorkspace } from "./FanWorkspace";
import { useCoolingControl } from "./useCoolingControl";
import { ParameterDescriptions } from "./ParameterHelp";
import { FAN_HELP } from "./parameterDescriptions";

export function CoolingPanel({ active, revision, onSettled }: {
  active: boolean; revision: number; onSettled: (source: "fan" | "options") => void;
}) {
  const model = useCoolingControl(active, revision, onSettled);
  const { profile } = model;
  const [editing, setEditing] = useState(false);
  const fanWorkspaceId = useId();
  const editButton = useRef<HTMLButtonElement>(null);
  const curveHeading = useRef<HTMLHeadingElement>(null);
  if (editing && !profile?.customSupported) setEditing(false);
  const focusCurveHeading = useCallback((heading: HTMLHeadingElement | null) => {
    curveHeading.current = heading;
    heading?.focus();
  }, []);
  const openEditor = () => {
    if (editing) curveHeading.current?.focus();
    else if (model.canEditCurves && !model.busy) setEditing(true);
  };
  const closeEditor = () => {
    setEditing(false);
    if (active) editButton.current?.focus();
  };
  const curveAction = profile?.customSupported && <Button ref={editButton} size="compact" variant="secondary"
    aria-controls={fanWorkspaceId} aria-expanded={editing} disabled={!editing && (!model.canEditCurves || model.busy)}
    onClick={openEditor}>编辑曲线</Button>;
  const errors = [model.actionError, model.fanError, profile?.customError, model.options.error].filter(Boolean);
  const warnings = [...new Set([...(profile?.warnings ?? []), ...(model.options.value?.warnings ?? [])])];
  return (
    <Surface as="section" variant="group" className="ccc-panel" aria-label="散热控制" aria-busy={model.busy || undefined}>
      <div className="ccc-panel-head">
        <h2 className="ccc-panel-title"><Icon name="fan" />散热控制</h2>
        <div className="ccc-module-tools">
          {model.writing && <Loading embedded label="正在应用…" />}
          <Button icon="refresh" variant="ghost" size="icon-compact" aria-label="重新读取散热控制" disabled={model.busy} onClick={model.retry} />
          <DetailsDialog title="风扇说明与诊断">
            <section><h3>参数说明</h3><ParameterDescriptions items={FAN_HELP} /></section>
            {(profile || warnings.length > 0) && <section><h3>安全诊断</h3>{profile && <p className="ccc-control-caption">{profile.safetyStatus}</p>}
              {warnings.map(warning => <p key={warning} className="ccc-control-caption is-warn"><Icon name="warning" size={16} />{warning}</p>)}
            </section>}
          </DetailsDialog>
        </div>
      </div>
      {!!errors.length && <div role="alert" className="ccc-control-error">{[...new Set(errors)].join("；")}
        <Button icon="refresh" variant="ghost" size="compact" type="button" disabled={model.busy} onClick={model.retry}>重试</Button></div>}
      <OemConflictAction issues={[model.actionError, model.options.value?.blockedReason]} disabled={model.busy} onClosed={model.retry} />
      {((model.fanError && profile) || (model.options.error && model.options.value)) &&
        <p className="ccc-control-caption">显示上次读取的设置。重新读取后可修改。</p>}
      {profile ? <div className="ccc-cooling-layout">
        <CoolingStrategy model={model} profile={profile} active={active} revision={revision} curveAction={curveAction} />
        <FanWorkspace model={model} profile={profile} active={active} editing={editing} id={fanWorkspaceId}
          headingRef={focusCurveHeading} onClose={closeEditor} />
      </div> : !errors.length && <Loading label="正在读取…" />}
    </Surface>
  );
}
