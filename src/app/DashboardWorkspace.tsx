/**
 * 监控仪表盘（Phase 1）：订阅后端 1Hz hardware-snapshot 事件，展示
 * CPU/GPU/风扇/内存实时读数。active=false 时不订阅（副作用只在可见页运行）。
 * 读数 stale（回声检测）显示为「读数异常」；与 HWiNFO 对齐是验收标准。
 */
import { useEffect, useRef, useState } from "react";
import {
  getHardwareSnapshot,
  onHardwareSnapshot,
  type HardwareSnapshot,
  type Reading,
} from "../api/client";
import { FactList, type Fact } from "../components/FactList";

function formatReading(reading: Reading | null, unit: string, digits = 0): string {
  if (!reading || !reading.ok || reading.stale) return "读数异常";
  return `${reading.value.toFixed(digits)} ${unit}`;
}

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
  const temps = samples.flatMap((sample) => [sample.cpuTemp, sample.gpuTemp]).filter((t): t is number => t != null);
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
        温度历史（最近 {Math.round(samples.length / 60)} 分钟 · 上限 {maxTemp}°C）
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
        const cpu = snap.cpuTempC?.ok && !snap.cpuTempC.stale ? snap.cpuTempC.value : null;
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
    { label: "CPU 温度", value: formatReading(snapshot.cpuTempC, "°C") },
    {
      label: "CPU 频率",
      value:
        snapshot.cpuFreqMaxMhz?.ok && !snapshot.cpuFreqMaxMhz.stale
          ? `${formatReading(snapshot.cpuFreqMhz, "MHz")} / ${formatReading(snapshot.cpuFreqMaxMhz, "MHz")}`
          : formatReading(snapshot.cpuFreqMhz, "MHz"),
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
  }
  gpuFacts.push({
    label: "GPU 温度 (DCHU)",
    value: formatReading(snapshot.gpuTempC, "°C"),
    danger: snapshot.gpuTempC?.stale === true,
  });
  gpuFacts.push({
    label: "GPU 频率 (DCHU)",
    value: formatReading(snapshot.gpuFreqMhz, "MHz"),
  });
  gpuFacts.push({
    label: "内存用量",
    value: `${formatReading(snapshot.memoryUsedMb, "MB")} `,
  });

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
      <h3 className="ccc-group-title">GPU{snapshot.gpuTempC?.stale ? "（DCHU 通道读数异常，以下为 NVML 事实）" : ""}</h3>
      <FactList facts={gpuFacts} />
      <p style={{ color: "var(--ccc-text-muted)", fontSize: "var(--ccc-text-sm)" }}>
        采样于 {new Date(snapshot.sampledAtMs).toLocaleTimeString("zh-CN")} · 1Hz ·
        验收基准：与 HWiNFO 读数对齐（Phase 1）
      </p>
    </div>
  );
}
