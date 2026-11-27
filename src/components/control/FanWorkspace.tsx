import { useId, useState, type Ref } from "react";
import type { FanCurveChanges, FanId, FanProfile } from "../../api/client";
import { Button } from "../Button";
import { Surface } from "../Surface";
import { Tooltip } from "../Tooltip";
import { FanCurveEditor } from "./FanCurveEditor";
import { FanCurveSection } from "./FanCurveSection";
import { FanLiveReadings } from "./FanLiveReadings";
import { RpmCurveEditor } from "./RpmCurveEditor";
import { normalizeDraft, sameDraft } from "./curve";
import { sameRpmCurve } from "./rpmCurve";
import type { CoolingControl } from "./useCoolingControl";

type EditorProps = {
  model: CoolingControl;
  profile: FanProfile;
  active: boolean;
  headingRef: Ref<HTMLHeadingElement>;
  onClose: () => void;
};

export function FanWorkspace({ id, editing, ...editor }: EditorProps & { id: string; editing: boolean }) {
  const { profile, active } = editor;
  const fans = profile.curveKind === "rpmTable" ? profile.rpmFans : profile.fans.filter(fan => fan.present);
  return <Surface as="section" id={id} variant="inset" className="ccc-cooling-pane ccc-fan-workspace" aria-label="风扇实况与曲线">
    {editing ? <FanCurveConfiguration {...editor} /> :
      <FanLiveReadings active={active} ids={fans.map(fan => fan.id)} ratedFans={profile.rpmFans} />}
  </Surface>;
}

function changeCurve<K extends FanId, T>(curves: Partial<Record<K, T>>, id: K, value: T | undefined) {
  const next = { ...curves };
  if (value === undefined) delete next[id];
  else next[id] = value;
  return next;
}

function FanCurveConfiguration({ model, profile, active, headingRef, onClose }: EditorProps) {
  const [source] = useState(() => profile);
  const revision = source.custom?.revision ?? "";
  const saved = source.custom?.saved;
  const [changes, setChanges] = useState<FanCurveChanges>(() => ({ kind: profile.curveKind, curves: {} }));
  const fieldsId = useId();
  const [frontFan, setFrontFan] = useState<FanId | null>(null);
  const fieldId = (fan: FanId) => `${fieldsId}-${fan}`;
  const chooseFan = (fan: FanId) => {
    setFrontFan(fan);
    const fields = document.getElementById(fieldId(fan));
    fields?.focus({ preventScroll: true });
    fields?.scrollIntoView({ block: "nearest" });
  };
  const selection = { frontFan, fieldId, chooseFan };
  const rpm = changes.kind === "rpmTable";
  const twoPointFans = source.fans.filter(fan => fan.present);
  const fans = rpm ? source.rpmFans : twoPointFans;
  const minTemp = rpm ? 0 : Math.min(30, ...twoPointFans.map(fan => Math.floor(fan.t1 / 10) * 10));
  const changed = profile.custom?.revision !== revision;
  const editable = active && model.canEditCurves && !changed;
  const dirty = Object.keys(changes.curves).length > 0;
  const apply = async () => {
    if (!editable || !dirty || model.busy) return;
    if (await model.saveCurves(revision, changes)) onClose();
  };

  return <>
    <h3 ref={headingRef} tabIndex={-1}>风扇曲线</h3>
    <FanLiveReadings active={active} ids={fans.map(fan => fan.id)} ratedFans={profile.rpmFans} selection={selection} />
    <FanCurveSection rpm={rpm} minTemp={minTemp} fanCount={fans.length} selection={selection} onActivate={setFrontFan}>
      {layout => fans.map(fan => {
        const props = { ...layout, id: fan.id, source: revision, editable, busy: model.writing };
        if ("points" in fan && changes.kind === "rpmTable") {
          const current = (saved?.kind === "rpmTable" && saved.curves[fan.id]) || fan.points;
          return <RpmCurveEditor key={fan.id} {...props} current={current} maximumRpm={fan.maximumRpm}
            draft={changes.curves[fan.id] ?? current} onChange={next => setChanges(previous => previous.kind === "rpmTable"
              ? { kind: "rpmTable", curves: changeCurve(previous.curves, fan.id, sameRpmCurve(current, next) ? undefined : next) }
              : previous)} />;
        }
        if ("points" in fan || changes.kind !== "twoPoint") return null;
        const current = normalizeDraft(fan, (saved?.kind === "twoPoint" && saved.curves[fan.id]) || fan);
        return <FanCurveEditor key={fan.id} {...props} anchors={fan} current={current} draft={changes.curves[fan.id] ?? current}
          onChange={next => setChanges(previous => previous.kind === "twoPoint"
            ? { kind: "twoPoint", curves: changeCurve(previous.curves, fan.id, sameDraft(current, next) ? undefined : next) }
            : previous)} />;
      })}
    </FanCurveSection>
    {changed && <p role="status" className="ccc-control-caption">风扇曲线配置已变化，请重新打开编辑。</p>}
    <div className="ccc-actions ccc-fan-curve-actions">
      <Button size="compact" variant="secondary" disabled={model.writing} onClick={onClose}>取消</Button>
      <Tooltip content="保存自定义曲线，选择「自定义」时写入固件">
        <Button size="compact" disabled={!editable || !dirty || model.busy} onClick={() => void apply()}>应用</Button>
      </Tooltip>
    </div>
  </>;
}
