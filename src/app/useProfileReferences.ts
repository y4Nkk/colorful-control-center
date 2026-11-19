import { useEffect, useState } from "react";
import { getStartupProfile, readPowerProfilePolicy, type ProfileMeta } from "../api/client";
import { POWER_RULES } from "../components/control/usePowerProfilePolicy";

export interface ProfileReference {
  label: string;
  name: string;
  revision: string;
}

export function useProfileReferences(active: boolean, profiles: ProfileMeta[]) {
  const [value, setValue] = useState<{ catalog: ProfileMeta[]; rules: ProfileReference[]; issue: string | null }>();
  useEffect(() => {
    if (!active || !profiles.length) return;
    let disposed = false;
    Promise.allSettled([getStartupProfile(), readPowerProfilePolicy()]).then(([startup, power]) => {
      if (disposed) return;
      const next: ProfileReference[] = [];
      const errors: string[] = [];
      if (startup.status === "rejected") errors.push(String(startup.reason));
      else if (startup.value.selection) next.push({
        label: "启动时",
        name: startup.value.selection.profile,
        revision: startup.value.selection.digest,
      });
      if (power.status === "rejected") errors.push(String(power.reason));
      else {
        if (power.value.configurationError) errors.push(power.value.configurationError);
        for (const { key, label } of POWER_RULES) {
          const binding = power.value.record?.[key];
          if (binding) next.push({ label, name: binding.profile, revision: binding.digest });
        }
      }
      setValue({ catalog: profiles, rules: next, issue: errors.join("；") || null });
    });
    return () => { disposed = true; };
  }, [active, profiles]);
  const current = value?.catalog === profiles ? value : undefined;
  return { rules: current?.rules ?? [], rulesError: current?.issue ?? null, rulesLoading: !!profiles.length && !current };
}
