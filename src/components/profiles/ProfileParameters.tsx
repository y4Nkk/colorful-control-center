import { useId, useMemo, useState } from "react";
import type { ManualProfilePreview, ProfileSettings } from "../../api/client";
import { Button } from "../Button";
import { SegmentedControl } from "../SegmentedControl";
import { ProfilePreview } from "./ProfilePreview";

const parameterSources = [
  { value: "target", label: "档案目标" },
  { value: "before", label: "应用前已读取" },
] as const;

export function ProfileParameters({ settings, review, label = "查看全部设置" }: {
  settings: ProfileSettings; review?: ManualProfilePreview | null; label?: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const [source, setSource] = useState("target");
  const id = useId();
  const before = useMemo(() => {
    if (!review?.before || !expanded) return null;
    const keys = Object.keys(settings) as (keyof ProfileSettings)[];
    const selected = keys.map(key => [key, settings[key] == null ? null : review.before?.[key]] as const);
    return selected.some(([, value]) => value != null) ? Object.fromEntries(selected) as unknown as ProfileSettings : null;
  }, [expanded, review?.before, settings]);
  const showingCurrent = source === "before" && before !== null;
  const targets = showingCurrent ? null : review;
  return <div className="ccc-profile-disclosure">
    <Button icon={expanded ? "chevron-down" : "chevron-right"} variant="ghost" size="compact"
      aria-expanded={expanded} aria-controls={id} onClick={() => setExpanded(value => !value)}>{label}</Button>
    <div id={id} hidden={!expanded}>
      {expanded && <>
        {before && <SegmentedControl className="ccc-profile-parameter-sources" label="档案参数来源"
          options={parameterSources} value={source} onChange={setSource} />}
        <ProfilePreview settings={showingCurrent ? before : settings} modeTarget={targets?.modeTarget ?? null}
          displayReview={targets?.displayReview ?? null} gpuPowerReview={targets?.gpuPowerReview ?? null}
          graphicsModeReview={targets?.graphicsModeReview ?? null} />
      </>}
    </div>
  </div>;
}
