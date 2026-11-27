import { useCallback, useEffect, useRef, useState } from "react";
import { applyKeyboardLighting, readKeyboardLighting, type KeyboardLightingProfile, type KeyboardLightingRequest,
  type KeyboardEffect, type KeyboardZones, type RgbColor, type KeyboardApplyOutcome } from "../../api/client";
import { toast } from "../use-toast";
import { preserveEditedFields } from "./settings-draft";

export const EFFECTS: { id: KeyboardEffect; label: string }[] = [
  { id: "solid", label: "同色" }, { id: "zones", label: "分区" }, { id: "breath", label: "呼吸" },
  { id: "cycle", label: "循环" }, { id: "wave", label: "波浪" }, { id: "dance", label: "舞动" },
  { id: "tempo", label: "节奏" }, { id: "flash", label: "闪烁" }, { id: "random", label: "随机" },
];
export const KEYBOARD_ZONE_NAMES = ["左区", "中区", "右区"] as const;
export const usesZoneColors = (effect: KeyboardEffect | null) => effect === "zones" || effect === "breath";
export type LightingFamily = "zoneRgb" | "white";

type Draft = { brightness: number; effect: KeyboardEffect | null; allColor: RgbColor; zones: KeyboardZones };
const emptyDraft = (): Draft => ({ brightness: 0, effect: null, allColor: { r: 0, g: 0, b: 0 },
  zones: [0, 1, 2].map(() => ({ enabled: true, color: { r: 0, g: 0, b: 0 } })) as KeyboardZones });
const fromProfile = (profile: KeyboardLightingProfile): Draft => profile.current ? {
  brightness: profile.current.enabled ? profile.current.brightnessLevel : 0, effect: profile.current.effect,
  allColor: profile.current.allColor, zones: profile.current.zones,
} : emptyDraft();
export function useKeyboardLighting(active: boolean, revision = 0) {
  const [profile, setProfile] = useState<KeyboardLightingProfile | null>(), [draft, setDraft] = useState<Draft>(emptyDraft);
  const [error, setError] = useState<string | null>(null), [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null);
  const accepted = useRef<Draft | null>(null), running = useRef(false), generation = useRef(0);
  const reading = useRef<Promise<KeyboardLightingProfile> | null>(null);
  const load = useCallback(async (reset = false, afterWrite = false) => {
    if (reading.current || running.current && !afterWrite) return;
    const id = ++generation.current, pending = readKeyboardLighting(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      const before = accepted.current, nextDraft = fromProfile(next); accepted.current = nextDraft;
      setProfile(next); setReadError(null);
      setDraft(previous => {
        if (reset || !before) return nextDraft;
        const updated = preserveEditedFields(before, previous, nextDraft);
        updated.zones = nextDraft.zones.map((zone, index) => preserveEditedFields(before.zones[index], previous.zones[index], zone)) as KeyboardZones;
        return updated;
      });
    } catch (error) { if (id === generation.current) { setProfile(null); setReadError(String(error)); } throw error; }
    finally { if (reading.current === pending) reading.current = null; }
  }, []);
  useEffect(() => {
    let disposed = false;
    if (active) void (async () => { await reading.current?.catch(() => undefined); if (!disposed) await load(); })().catch(() => {});
    return () => { disposed = true; ++generation.current; };
  }, [active, revision, load]);
  const reload = () => { setError(null); void load().catch(() => {}); };
  const run = async (action: () => Promise<KeyboardApplyOutcome>, reset = true) => {
    if (!active || running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try {
      await reading.current?.catch(() => undefined);
      const result = await action();
      if (!result.ok) throw new Error([result.message, result.rollbackError].filter(Boolean).join("；"));
      await load(reset, true); toast({ kind: "success", title: "键盘灯设置已回读" });
    } catch (error) { setError(String(error)); await load(false, true).catch(() => {}); }
    finally { running.current = false; setBusy(false); }
  };
  const family: LightingFamily | null = profile?.detectedFamily === "white" || profile?.detectedFamily === "zoneRgb" ? profile.detectedFamily : null;
  const current = profile?.current ? fromProfile(profile) : null;
  const request: KeyboardLightingRequest | null = family === "white" ? { family, brightness: draft.brightness }
    : family === "zoneRgb" && current?.effect ? { family, ...current, effect: current.effect,
      allColor: current.effect === "solid" ? draft.allColor : current.allColor,
      zones: usesZoneColors(current.effect) ? draft.zones : current.zones } : null;
  const dirty = !!current && (family === "white" ? draft.brightness !== current.brightness
    : current.effect === "solid" ? JSON.stringify(current.allColor) !== JSON.stringify(draft.allColor)
    : usesZoneColors(current.effect) && JSON.stringify(current.zones) !== JSON.stringify(draft.zones));
  const apply = (update: Partial<Draft>) => {
    if (!current || family !== "zoneRgb" || !profile?.available
      || Object.entries(update).every(([key, value]) => JSON.stringify(value) === JSON.stringify(current[key as keyof Draft]))) return;
    const next = { ...current, ...update };
    const effect = next.effect;
    if (effect) void run(() => applyKeyboardLighting({ family, ...next, effect }), false);
  };
  return { profile: profile ?? null, loaded: profile !== undefined, family, ...draft, request, dirty, error: error ?? readError, busy, reload, run,
    setBrightness: (brightness: number) => setDraft(previous => ({ ...previous, brightness })),
    chooseEffect: (effect: KeyboardEffect) => apply({ effect }),
    chooseBrightness: (brightness: number) => apply({ brightness }),
    toggleZone: (index: number, enabled: boolean) => {
      if (!current || !usesZoneColors(current.effect)) return;
      const zones: KeyboardZones = [...current.zones];
      zones[index] = { ...zones[index], enabled };
      apply({ zones });
    },
    setAllColor: (allColor: RgbColor) => setDraft(previous => ({ ...previous, allColor })),
    setZones: (zones: KeyboardZones) => setDraft(previous => ({ ...previous, zones })) };
}
