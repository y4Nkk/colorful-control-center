import { useId, useMemo, useState } from "react";
import type { ManualProfilePreview, ProfileSettings } from "../../api/client";
import { Button } from "../Button";
import { Tabs } from "../Tabs";
import { ProfilePreview } from "./ProfilePreview";

type ParameterSource = "target" | "before";

export function ProfileParameters({ settings, review, label = "查看全部设置" }: {
  settings: ProfileSettings; review?: ManualProfilePreview | null; label?: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const [source, setSource] = useState<ParameterSource>("target");
  const id = useId();
  const before = useMemo(() => {
    if (!review?.before || !expanded) return null;
    const keys = Object.keys(settings) as (keyof ProfileSettings)[];
    const selected = keys.map(key => [key, settings[key] == null ? null : review.before?.[key]] as const);
    return selected.some(([, value]) => value != null) ? Object.fromEntries(selected) as unknown as ProfileSettings : null;
  }, [expanded, review?.before, settings]);
  const target = <ProfilePreview settings={settings} modeTarget={review?.modeTarget ?? null}
    displayReview={review?.displayReview ?? null} gpuPowerReview={review?.gpuPowerReview ?? null}
    graphicsModeReview={review?.graphicsModeReview ?? null} />;
  return <div className="ccc-profile-disclosure">
    <Button icon={expanded ? "chevron-down" : "chevron-right"} variant="ghost" size="compact"
      aria-expanded={expanded} aria-controls={id} onClick={() => setExpanded(value => !value)}>{label}</Button>
    <div id={id} hidden={!expanded}>
      {expanded && (before ? <div className="ccc-profile-parameter-views">
        <Tabs label="档案参数来源" value={source} onChange={setSource} tabs={[
          { value: "target", label: "档案目标", content: target },
          { value: "before", label: "应用前已读取", content: <ProfilePreview settings={before} modeTarget={null}
            displayReview={null} gpuPowerReview={null} graphicsModeReview={null} /> },
        ]} />
      </div> : target)}
    </div>
  </div>;
}
