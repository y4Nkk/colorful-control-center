import { useEffect, useState } from "react";
import { readAutomaticProfileApplication, type ProfileApplication } from "../../api/client";
import { useProfileApplicationState } from "../../app/profile-application";

export function useAutomaticProfileReport(active: boolean) {
  const { report } = useProfileApplicationState();
  const [latest, setLatest] = useState<ProfileApplication | null>(null), [error, setError] = useState<string | null>(null);
  const automatic = Boolean(report && ["ac", "battery", "resume"].includes(report.origin)), id = report?.id, revision = report?.revision;
  useEffect(() => {
    if (active && automatic && report) { setLatest(report); setError(null); }
  }, [active, automatic, report]);
  useEffect(() => {
    if (!active || automatic) return;
    let disposed = false;
    void readAutomaticProfileApplication().then(value => { if (!disposed) { setLatest(value); setError(null); } })
      .catch(error => { if (!disposed) setError(String(error)); });
    return () => { disposed = true; };
  }, [active, automatic, id, revision]);
  return { report: latest, error };
}
