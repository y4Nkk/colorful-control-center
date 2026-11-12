/**
 * 监控仪表盘（Phase 1 接入真实监控链路）。当前为占位 + IPC 冒烟：
 * 展示后端版本证明 api/client.ts → commands → 前端全链路可用。
 */
import { useEffect, useState } from "react";
import { getBackendInfo, type BackendInfo } from "../api/client";
import { PagePlaceholder } from "../components/PagePlaceholder";

export function DashboardWorkspace({ active }: { active: boolean }) {
  const [info, setInfo] = useState<BackendInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    getBackendInfo()
      .then((result) => {
        if (!cancelled) {
          setInfo(result);
          setError(null);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [active]);

  return (
    <PagePlaceholder
      title="监控仪表盘"
      phase="Phase 1"
      description="接入 CPU / GPU / 风扇 / 温度实时监控，读数与 HWiNFO 对齐后开启写入能力。"
    >
      <p>
        {error
          ? `后端读取失败：${error}`
          : info
            ? (
              <>
                后端通路正常：<span className="ccc-mono">{info.name}</span> v
                <span className="ccc-mono">{info.version}</span>
              </>
            )
            : "正在读取后端信息……"}
      </p>
    </PagePlaceholder>
  );
}
