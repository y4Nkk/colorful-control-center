/** 性能档案（Phase 3 接入：PL / EPP / GPU 偏移 / 风扇曲线打包一键切换）。 */
import { PagePlaceholder } from "../components/PagePlaceholder";

export function ProfilesWorkspace(_props: { active: boolean }) {
  return (
    <PagePlaceholder
      title="性能档案"
      phase="Phase 3"
      description="将功耗墙、偏移电压、GPU 偏移与风扇曲线打包为档案，一键切换、开机自启并应用。"
    />
  );
}
