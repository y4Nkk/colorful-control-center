import { FAN_LABELS, type FanId, type ProfileSettings } from "../../api/client";
import { rpmStepPath } from "../control/rpmCurve";
import { sameProfileValue } from "./profile-differences";

function curves(settings: ProfileSettings | null) {
  if (!settings) return [];
  return [
    ...Object.entries(settings.fanCurves ?? {}).map(([id, points]) => ({
      id, kind: "twoPoint", unit: "占空比", points,
      path: `M${points.t2},${100 - points.d2Pct} L${points.t3},${100 - points.d3Pct} L100,0`,
    })),
    ...Object.entries(settings.fanRpmCurves ?? {}).map(([id, points]) => ({
      id, kind: "rpm", unit: "最大转速百分比", points, path: rpmStepPath(points),
    })),
    ...Object.entries(settings.fanSoftwareCurves ?? {}).map(([id, curve]) => ({
      id, kind: "software", unit: `软件占空比 · 回差 ${curve!.hysteresisC}° · 斜坡 ${curve!.rampPctPerS}%/秒`, points: curve,
      path: `M0,${100 - curve!.points[0].dutyPct} ` + curve!.points.map(point => `L${point.tempC},${100 - point.dutyPct}`).join(" "),
    })),
  ];
}

export function ProfileCurves({ settings, before, emptyMessage }: {
  settings: ProfileSettings; before?: ProfileSettings | null; emptyMessage?: string;
}) {
  const current = curves(before ?? null);
  const plots = curves(settings).flatMap(target => {
    const sameKind = current.find(item => item.id === target.id && item.kind === target.kind);
    return sameKind && sameProfileValue(sameKind.points, target.points) ? [] : [{ target, current: sameKind }];
  });
  if (!plots.length) return emptyMessage ? <p className="ccc-control-caption">{emptyMessage}</p> : null;
  return <div className="ccc-profile-curves">{plots.map(({ target, current }) => {
    const label = FAN_LABELS[target.id as FanId];
    const description = before === undefined ? "档案曲线" : current ? "当前与档案曲线" : "档案曲线 · 当前无可比曲线";
    return <figure key={target.id}>
      <figcaption>{label} · {target.unit}</figcaption>
      <svg viewBox="0 0 100 100" preserveAspectRatio="none" role="img" aria-label={`${label} ${description}`}>
        {current && <path className="ccc-profile-curve-current" d={current.path} vectorEffect="non-scaling-stroke" />}
        <path className="ccc-profile-curve-target" d={target.path} vectorEffect="non-scaling-stroke" />
      </svg>
      <div className="ccc-profile-curve-axis" aria-hidden="true"><span>0°C</span><span>100°C / 100%</span></div>
      {before !== undefined && <div className="ccc-profile-curve-key">
        {current ? <><span data-curve="current">当前</span><span data-curve="target">档案</span></> : description}
      </div>}
    </figure>;
  })}</div>;
}
