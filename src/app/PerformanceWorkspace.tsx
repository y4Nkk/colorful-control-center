/**
 * 性能调校（Phase 3）：PL1/PL2 功耗墙 + 偏移电压（undervolt）。
 * 通道：持久化（page6，OEM 同款，默认）与运行时（MSR 直写，需 PawnIO）。
 * 所有写入前二次确认；失败经 toast 反馈。undervolt 钳位 −150…+50 mV。
 */
import { useEffect, useState } from "react";
import {
  applyPlPersist,
  applyPlRuntime,
  applyVoltagePersist,
  applyVoltageRuntime,
  readPowerProfile,
  type PowerProfile,
} from "../api/client";
import { FactList } from "../components/FactList";
import { ConfirmButton } from "../components/ConfirmButton";
import { toast } from "../components/use-toast";

export function PerformanceWorkspace({ active }: { active: boolean }) {
  const [profile, setProfile] = useState<PowerProfile | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pl1, setPl1] = useState("");
  const [pl2, setPl2] = useState("");
  const [coreMv, setCoreMv] = useState("");
  const [gtMv, setGtMv] = useState("");

  useEffect(() => {
    if (!active) return;
    readPowerProfile()
      .then((p) => {
        setProfile(p);
        setError(null);
        setPl1(String(Math.round(p.pl1W ?? 0)));
        setPl2(String(Math.round(p.pl2W ?? 0)));
        setCoreMv(String(Math.round(p.coreOffsetMv ?? 0)));
        setGtMv(String(Math.round(p.gtOffsetMv ?? 0)));
      })
      .catch((cause: unknown) => setError(cause instanceof Error ? cause.message : String(cause)));
  }, [active]);

  const reload = () =>
    readPowerProfile().then(setProfile).catch(() => {});

  const num = (v: string) => {
    const n = Number(v);
    return Number.isFinite(n) ? n : null;
  };

  if (error) {
    return <div className="ccc-dashboard"><div className="ccc-warnings">读取失败：{error}</div></div>;
  }
  if (!profile) {
    return <div className="ccc-dashboard"><div className="ccc-warnings">正在读取性能配置……</div></div>;
  }

  const factDefaults: { label: string; value: string }[] = [
    { label: "出厂 PL1/PL2", value: `${profile.defaultPl1W?.toFixed(0) ?? "?"} / ${profile.defaultPl2W?.toFixed(0) ?? "?"} W` },
    { label: "出厂核心电压偏移", value: `${profile.defaultCoreOffsetMv ?? 0} mV` },
    { label: "运行时 MSR 通道", value: profile.msrRuntimeAvailable ? "可用（PawnIO 已装）" : "不可用（需管理员运行 run-admin-probe.ps1）" },
  ];

  return (
    <div className="ccc-dashboard">
      {profile.warnings.length > 0 && (
        <div className="ccc-warnings">{profile.warnings.map((w) => <div key={w}>{w}</div>)}</div>
      )}

      <h3 className="ccc-group-title">功耗墙（PL1 / PL2）</h3>
      <div className="ccc-placeholder" style={{ display: "flex", gap: 24, alignItems: "center" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <NumberRow label="PL1 (W)" value={pl1} onChange={setPl1} />
          <NumberRow label="PL2 (W)" value={pl2} onChange={setPl2} />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <ConfirmButton
            label="应用（持久化 page6）"
            confirmLabel="确认写入？"
            onConfirm={() => {
              const v1 = num(pl1);
              const v2 = num(pl2);
              applyPlPersist(v1, v2).then((r) => {
                if (r.ok) {
                  toast({ kind: "success", title: r.message });
                  reload();
                } else {
                  toast({ kind: "error", title: "写入失败", description: r.message });
                }
              });
            }}
          />
          <ConfirmButton
            label="应用（运行时 MSR）"
            confirmLabel="确认写入？"
            onConfirm={() => {
              const v1 = num(pl1);
              const v2 = num(pl2);
              applyPlRuntime(v1, v2).then((r) => {
                if (r.ok) {
                  toast({ kind: "success", title: r.message });
                } else {
                  toast({ kind: "error", title: "写入失败", description: r.message });
                }
              });
            }}
          />
        </div>
      </div>

      <h3 className="ccc-group-title">偏移电压（undervolt）</h3>
      <div className="ccc-placeholder" style={{ display: "flex", gap: 24, alignItems: "center" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <NumberRow label="核心 (mV)" value={coreMv} onChange={setCoreMv} />
          <NumberRow label="GT (mV)" value={gtMv} onChange={setGtMv} />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <ConfirmButton
            label="应用（持久化 page6）"
            confirmLabel="确认写入？"
            onConfirm={() => {
              const c = num(coreMv);
              const g = num(gtMv);
              applyVoltagePersist(c === null ? null : Math.round(c), g === null ? null : Math.round(g)).then((r) => {
                if (r.ok) {
                  toast({ kind: "success", title: r.message });
                  reload();
                } else {
                  toast({ kind: "error", title: "写入失败", description: r.message });
                }
              });
            }}
          />
          <ConfirmButton
            label="核心（运行时 MSR）"
            confirmLabel="确认写入？"
            onConfirm={() => {
              const c = num(coreMv);
              if (c === null) return;
              applyVoltageRuntime(0, Math.round(c)).then((r) => {
                if (r.ok) {
                  toast({ kind: "success", title: r.message });
                } else {
                  toast({ kind: "error", title: "写入失败", description: r.message });
                }
              });
            }}
          />
        </div>
      </div>

      <h3 className="ccc-group-title">基准事实</h3>
      <FactList facts={factDefaults} />
      <p style={{ color: "var(--ccc-text-muted)", fontSize: "var(--ccc-text-sm)" }}>
        持久化通道与 OEM Control Center 的 CPU_OC 完全同款（page6 + IsSaved）；生效时机取决于
        OEM 后台服务/固件。运行时通道要求 PawnIO 驱动，写入即时生效。
      </p>
    </div>
  );
}

function NumberRow({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <span style={{ color: "var(--ccc-text-muted)", minWidth: 72 }}>{label}</span>
      <input className="ccc-input" type="number" value={value} onChange={(e) => onChange(e.target.value)} />
    </label>
  );
}
