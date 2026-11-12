/**
 * 风扇曲线（Phase 2，OEM 曲线模式）：每风扇 T2/D2、T3/D3 可调
 * （T1/D1 与 100%/100°C 由固件固定），写入前二次确认；固件执行曲线，
 * 软件退出不影响。写入失败走 toast（唯一通知面）。
 */
import { useEffect, useState } from "react";
import {
  applyFanCurve,
  FAN_MODES,
  readFanProfile,
  restoreFanDefault,
  setFanMode,
  type CurvePointsDto,
  type FanPointSet,
  type FanProfile,
} from "../api/client";
import { ConfirmButton } from "../components/ConfirmButton";
import { toast } from "../components/use-toast";

/** 单风扇可调点编辑态 */
type EditedPoints = { t2: string; d2: string; t3: string; d3: string };

function toEdited(set: FanPointSet): EditedPoints {
  return {
    t2: String(Math.round(set.t2)),
    d2: String(Math.round(set.d2Pct)),
    t3: String(Math.round(set.t3)),
    d3: String(Math.round(set.d3Pct)),
  };
}

function parseEdited(edit: EditedPoints): CurvePointsDto | null {
  const t2 = Number(edit.t2);
  const d2 = Number(edit.d2);
  const t3 = Number(edit.t3);
  const d3 = Number(edit.d3);
  if ([t2, d2, t3, d3].some((v) => !Number.isFinite(v))) return null;
  return { t2, d2Pct: d2, t3, d3Pct: d3 };
}

/** 曲线预览：T1/D1 → T2/D2 → T3/D3 → 100/100 四锚点折线（SVG）。 */
function CurvePreview({ set, edit }: { set: FanPointSet; edit: EditedPoints }) {
  const t2 = Number(edit.t2) || set.t2;
  const d2 = Number(edit.d2) || set.d2Pct;
  const t3 = Number(edit.t3) || set.t3;
  const d3v = Number(edit.d3) || set.d3Pct;
  const points = [
    [set.t1, set.d1Pct],
    [t2, d2],
    [t3, d3v],
    [100, 100],
  ] as const;
  const toX = (t: number) => ((t - 30) / (100 - 30)) * 280;
  const toY = (d: number) => 120 - (d / 100) * 110;
  const path = points
    .map(([t, d], i) => `${i === 0 ? "M" : "L"}${toX(t).toFixed(1)},${toY(d).toFixed(1)}`)
    .join(" ");
  return (
    <svg width="300" height="130" role="img" aria-label="曲线预览">
      <rect x="0" y="0" width="300" height="130" fill="var(--ccc-content)" rx="8" />
      {[25, 50, 75].map((d) => (
        <line key={d} x1="10" x2="290" y1={toY(d)} y2={toY(d)} stroke="var(--ccc-hairline)" />
      ))}
      <path d={path} fill="none" stroke="var(--ccc-action)" strokeWidth="2" />
      {points.map(([t, d], i) => (
        <circle key={i} cx={toX(t)} cy={toY(d)} r="3.5" fill="var(--ccc-action)" />
      ))}
      <text x="150" y="128" textAnchor="middle" fontSize="10" fill="var(--ccc-text-muted)">
        温度 {set.t1}°C → 100°C
      </text>
    </svg>
  );
}

function NumberField({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <span style={{ color: "var(--ccc-text-muted)", minWidth: 56 }}>{label}</span>
      <input
        className="ccc-input"
        type="number"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  );
}

export function FansWorkspace({ active }: { active: boolean }) {
  const [profile, setProfile] = useState<FanProfile | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [edits, setEdits] = useState<Record<string, EditedPoints>>({});

  useEffect(() => {
    if (!active) return;
    readFanProfile()
      .then((p) => {
        setProfile(p);
        setError(null);
        setEdits(Object.fromEntries(p.fans.map((f) => [f.id, toEdited(f)])));
      })
      .catch((cause: unknown) => {
        setError(cause instanceof Error ? cause.message : String(cause));
      });
  }, [active]);

  const reload = () => {
    readFanProfile()
      .then((p) => {
        setProfile(p);
        setEdits(Object.fromEntries(p.fans.map((f) => [f.id, toEdited(f)])));
      })
      .catch(() => {});
  };

  const run = (action: () => Promise<{ ok: boolean; message: string }>, successText: string) => {
    action().then((result) => {
      if (result.ok) {
        toast({ kind: "success", title: successText });
        reload();
      } else {
        toast({ kind: "error", title: "风扇写入失败", description: result.message });
      }
    });
  };

  if (error) {
    return (
      <div className="ccc-dashboard">
        <div className="ccc-warnings">风扇配置读取失败：{error}</div>
      </div>
    );
  }
  if (!profile) {
    return (
      <div className="ccc-dashboard">
        <div className="ccc-warnings">正在读取风扇配置……</div>
      </div>
    );
  }

  const applyAll = () => {
    const dto = (id: string): CurvePointsDto | null => {
      const edit = edits[id];
      if (!edit) return null;
      return parseEdited(edit);
    };
    const cpu = dto("cpu");
    if (!cpu) {
      toast({ kind: "error", title: "输入无效", description: "曲线点必须是数字" });
      return;
    }
    run(
      () => applyFanCurve(cpu, dto("gpu1"), dto("gpu2")),
      "曲线已写入固件（模式：自定义）",
    );
  };

  return (
    <div className="ccc-dashboard">
      {!profile.customSupported && (
        <div className="ccc-warnings">本机固件未报告自定义曲线支持（cmd13 能力位）——模式切换仍可用。</div>
      )}
      <div className="ccc-facts" style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <span style={{ color: "var(--ccc-text-muted)" }}>当前模式：</span>
        {FAN_MODES.map((mode) => (
          <button
            key={mode.id}
            type="button"
            className={profile.currentMode === mode.id ? "ccc-button" : "ccc-nav-item"}
            style={profile.currentMode === mode.id ? undefined : { width: "auto", height: 32 }}
            onClick={() => run(() => setFanMode(mode.id), `模式已切换：${mode.label}`)}
          >
            {mode.label}
          </button>
        ))}
        <span style={{ flex: 1 }} />
        <ConfirmButton label="恢复默认曲线" confirmLabel="确认恢复默认？" onConfirm={() => run(restoreFanDefault, "已恢复固件默认曲线")} />
      </div>

      {profile.fans
        .filter((fan) => fan.present)
        .map((fan) => {
          const edit = edits[fan.id] ?? toEdited(fan);
          const update = (patch: Partial<EditedPoints>) =>
            setEdits((current) => ({ ...current, [fan.id]: { ...edit, ...patch } }));
          return (
            <div key={fan.id} className="ccc-placeholder" style={{ display: "flex", gap: 24, alignItems: "center" }}>
              <CurvePreview set={fan} edit={edit} />
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                <strong>{fan.id === "cpu" ? "CPU 风扇" : fan.id === "gpu1" ? "GPU 风扇 1" : "GPU 风扇 2"}</strong>
                <span style={{ color: "var(--ccc-text-muted)", fontSize: "var(--ccc-text-sm)" }}>
                  固件固定起点：T1={fan.t1}°C / D1={fan.d1Pct.toFixed(0)}%（终点 100°C / 100%）
                </span>
                <NumberField label="T2 (°C)" value={edit.t2} onChange={(v) => update({ t2: v })} />
                <NumberField label="D2 (%)" value={edit.d2} onChange={(v) => update({ d2: v })} />
                <NumberField label="T3 (°C)" value={edit.t3} onChange={(v) => update({ t3: v })} />
                <NumberField label="D3 (%)" value={edit.d3} onChange={(v) => update({ d3: v })} />
              </div>
            </div>
          );
        })}

      <div style={{ display: "flex", gap: 12 }}>
        <ConfirmButton label="应用曲线（写入固件）" confirmLabel="确认写入？" onConfirm={applyAll} />
        <span style={{ color: "var(--ccc-text-muted)", alignSelf: "center", fontSize: "var(--ccc-text-sm)" }}>
          曲线由固件执行——软件退出后依然生效；占空比下限 20% 防失速。
        </span>
      </div>
    </div>
  );
}
