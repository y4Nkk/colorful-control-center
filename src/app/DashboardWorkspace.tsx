/**
 * 监控仪表盘（Phase 1）：订阅后端 1Hz hardware-snapshot 事件。
 * CPU 温度 = DCHU cmd12 remote 换算；GPU = NVML（事实来源）；
 * 风扇 RPM/duty = cmd12 包读。历史曲线 keep-alive 保活。
 * 验收基准：与 HWiNFO 对齐（65W 桶换算若偏差大，在设置中校准桶参数）。
 */
import { useEffect, useRef, useState } from "react";
import {
  getHardwareSnapshot,
  onHardwareSnapshot,
  type HardwareSnapshot,
} from "../api/client";
import { FactList, type Fact } from "../components/FactList";

/** 历史采样环形缓冲（keep-alive 保活；最多 300 点 = 5 分钟 @1Hz）。 */
interface HistorySample {
  atMs: number;
  cpuTemp: number | null;
  gpuTemp: number | null;
}

const HISTORY_LIMIT = 300;

function HistoryChart({ samples }: { samples: HistorySample[] }) {
  if (samples.length < 2) return null;
  const width = 820;
  const height = 120;
  const temps = samples
    .flatMap((sample) => [sample.cpuTemp, sample.gpuTemp])
    .filter((t): t is number => t != null);
  const maxTemp = Math.max(60, ...temps) + 5;
  const toX = (index: number) => (index / (HISTORY_LIMIT - 1)) * width;
  const toY = (temp: number) => height - (temp / maxTemp) * (height - 16) - 4;
  const line = (pick: (sample: HistorySample) => number | null) => {
    let path = "";
    samples.forEach((sample, index) => {
      const value = pick(sample);
      if (value == null) return;
      path += `${path ? "L" : "M"}${toX(index).toFixed(1)},${toY(value).toFixed(1)}`;
    });
    return path;
  };
  return (
    <div>
      <h3 className="ccc-group-title">
        温度历史（最近 {Math.max(1, Math.round(samples.length / 60))} 分钟 · 上限 {maxTemp}°C）
      </h3>
      <svg width={width} height={height + 16} role="img" aria-label="温度历史曲线">
        <rect width={width} height={height} fill="var(--ccc-content)" rx="8" />
        {[40, 70, 90].map((t) =>
          t <= maxTemp ? (
            <g key={t}>
              <line x1="0" x2={width} y1={toY(t)} y2={toY(t)} stroke="var(--ccc-hairline)" />
              <text x="4" y={toY(t) - 3} fontSize="10" fill="var(--ccc-text-muted)">{t}°</text>
            </g>
          ) : null,
        )}
        <path d={line((s) => s.cpuTemp)} fill="none" stroke="var(--ccc-action)" strokeWidth="1.8" />
        <path d={line((s) => s.gpuTemp)} fill="none" stroke="var(--ccc-safe)" strokeWidth="1.8" />
        <text x={width - 6} y="14" textAnchor="end" fontSize="11" fill="var(--ccc-action)">CPU</text>
        <text x={width - 46} y="14" textAnchor="end" fontSize="11" fill="var(--ccc-safe)">GPU</text>
        <text x="4" y={height + 12} fontSize="10" fill="var(--ccc-text-muted)">
          {new Date(samples[0].atMs).toLocaleTimeString("zh-CN")}
        </text>
        <text x={width - 4} y={height + 12} textAnchor="end" fontSize="10" fill="var(--ccc-text-muted)">
          {new Date(samples[samples.length - 1].atMs).toLocaleTimeString("zh-CN")}
        </text>
      </svg>
    </div>
  );
}

export function DashboardWorkspace({ active }: { active: boolean }) {
  const [snapshot, setSnapshot] = useState<HardwareSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const historyRef = useRef<HistorySample[]>([]);
  const [history, setHistory] = useState<HistorySample[]>([]);

  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    getHardwareSnapshot()
      .then((snap) => {
        if (!cancelled) {
          setSnapshot(snap);
          setError(null);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
      });
    onHardwareSnapshot((snap) => {
      if (!cancelled) {
        setSnapshot(snap);
        setError(null);
        const cpu = snap.cpuTempC;
        const gpu = snap.nvidia?.tempC ?? null;
        historyRef.current = [
          ...historyRef.current,
          { atMs: snap.sampledAtMs, cpuTemp: cpu, gpuTemp: gpu },
        ].slice(-HISTORY_LIMIT);
        setHistory(historyRef.current);
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [active]);

  if (error) {
    return (
      <div className="ccc-dashboard">
        <div className="ccc-warnings">监控读取失败：{error}</div>
      </div>
    );
  }
  if (!snapshot) {
    return (
      <div className="ccc-dashboard">
        <div className="ccc-warnings">正在建立监控通道……</div>
      </div>
    );
  }

  const nvidia = snapshot.nvidia;
  const cpuFacts: Fact[] = [
    { label: "CPU 温度", value: snapshot.cpuTempC != null ? `${snapshot.cpuTempC.toFixed(0)} °C` : "—" },
    {
      label: "CPU remote 原始值",
      value: snapshot.cpuRemoteRaw != null ? `${snapshot.cpuRemoteRaw.toFixed(0)}` : "—",
    },
  ];
  const fanFacts: Fact[] = [
    {
      label: "CPU 风扇",
      value:
        snapshot.fan.cpuRpm != null
          ? `${snapshot.fan.cpuRpm.toFixed(0)} RPM · ${snapshot.fan.cpuDutyPct?.toFixed(0)}%`
          : "—",
    },
    {
      label: "GPU 风扇",
      value:
        snapshot.fan.gpuRpm != null
          ? `${snapshot.fan.gpuRpm.toFixed(0)} RPM · ${snapshot.fan.gpuDutyPct?.toFixed(0)}%`
          : "—",
    },
  ];
  const gpuFacts: Fact[] = [];
  if (nvidia) {
    gpuFacts.push({ label: "GPU 型号", value: nvidia.name || "—" });
    gpuFacts.push({
      label: "GPU 温度",
      value: nvidia.tempC != null ? `${nvidia.tempC.toFixed(0)} °C` : "—",
    });
    gpuFacts.push({
      label: "GPU 利用率",
      value: nvidia.utilizationPct != null ? `${nvidia.utilizationPct.toFixed(0)} %` : "—",
    });
    if (nvidia.memTotalMb != null) {
      gpuFacts.push({
        label: "显存",
        value: `${(nvidia.memUsedMb ?? 0).toFixed(0)} / ${nvidia.memTotalMb.toFixed(0)} MB`,
      });
    }
  } else {
    gpuFacts.push({ label: "GPU", value: "NVML 不可用", danger: true });
  }

  return (
    <div className="ccc-dashboard">
      {!snapshot.dchuAvailable && (
        <div className="ccc-warnings">
          InsydeDCHU 通道不可用：DCHU 读数全部缺失（OEM Control Center 被卸载或驱动未运行）。
        </div>
      )}
      {snapshot.warnings.length > 0 && (
        <div className="ccc-warnings">
          {snapshot.warnings.map((warning) => (
            <div key={warning}>{warning}</div>
          ))}
        </div>
      )}
      <FactList facts={cpuFacts} />
      <h3 className="ccc-group-title">风扇</h3>
      <FactList facts={fanFacts} />
      <HistoryChart samples={history} />
      <h3 className="ccc-group-title">GPU（NVML）</h3>
      <FactList facts={gpuFacts} />
      <p style={{ color: "var(--ccc-text-muted)", fontSize: "var(--ccc-text-sm)" }}>
        采样于 {new Date(snapshot.sampledAtMs).toLocaleTimeString("zh-CN")} · 1Hz ·
        验收基准：与 HWiNFO 对齐；CPU 换算桶（65W）如有偏差请回填 acceptance-checklist
      </p>
    </div>
  );
}
