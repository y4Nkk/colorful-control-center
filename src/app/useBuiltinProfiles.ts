import { useCallback, useEffect, useRef, useState } from "react";
import { readBuiltinProfiles, type BuiltinProfileCatalog, type BuiltinRefreshChoice } from "../api/client";

export function useBuiltinProfiles(active: boolean) {
  const [catalog, setCatalog] = useState<BuiltinProfileCatalog | null>(null);
  const [choice, setChoice] = useState<BuiltinRefreshChoice | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const generation = useRef(0);
  const reload = useCallback(async () => {
    const request = ++generation.current;
    setLoading(true);
    setError(null);
    try {
      const result = await readBuiltinProfiles();
      if (generation.current === request) setCatalog(result);
    } catch (error) {
      if (generation.current === request) setError(String(error));
    } finally {
      if (generation.current === request) setLoading(false);
    }
  }, []);
  useEffect(() => {
    if (!active) return;
    void reload();
    return () => { ++generation.current; };
  }, [active, reload]);
  return { catalog, choice, setChoice, error, loading, reload };
}
