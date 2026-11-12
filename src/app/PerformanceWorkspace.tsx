/** 性能调校（Phase 3 接入：PL1/PL2 功耗墙、偏移电压、GPU 频率偏移）。 */
import { PagePlaceholder } from "../components/PagePlaceholder";

export function PerformanceWorkspace(_props: { active: boolean }) {
  return (
    <PagePlaceholder
      title="性能调校"
      phase="Phase 3"
      description="PL1/PL2 独立可调、偏移电压（undervolt，Phase 0 先验证 MSR 0x150 锁状态）、GPU 核心/显存频率偏移。"
    />
  );
}
