/** 风扇曲线（Phase 2 接入：任意控制点 + 滞后回差 + 斜坡限制 + 看门狗）。 */
import { PagePlaceholder } from "../components/PagePlaceholder";

export function FansWorkspace(_props: { active: boolean }) {
  return (
    <PagePlaceholder
      title="风扇曲线"
      phase="Phase 2"
      description="自定义温度-转速曲线：CPU/GPU 温度取 max 驱动、滞后回差防抖、斜率限制防抽风，看门狗崩溃恢复。"
    />
  );
}
