import { useCallback, useEffect, useRef, useState } from "react";
import { readKeyboardLighting, type KeyboardLightingProfile, type KeyboardLightingRequest,
  type KeyboardEffect, type KeyboardZones, type RgbColor, type KeyboardApplyOutcome } from "../../api/client";
import { toast } from "../use-toast";

export const EFFECTS: { id: KeyboardEffect; label: string }[] = [
  { id: "solid", label: "同色" }, { id: "zones", label: "分区" }, { id: "breath", label: "呼吸" },
  { id: "cycle", label: "循环" }, { id: "wave", label: "波浪" }, { id: "dance", label: "舞动" },
  { id: "tempo", label: "节奏" }, { id: "flash", label: "闪烁" }, { id: "random", label: "随机" },
];
export type LightingFamily = "zoneRgb" | "white";
export const hexOf = (color: RgbColor) => `#${[color.r, color.g, color.b].map((v) => v.toString(16).padStart(2, "0")).join("")}`;
export const rgbOf = (hex: string): RgbColor => ({ r: parseInt(hex.slice(1, 3), 16), g: parseInt(hex.slice(3, 5), 16), b: parseInt(hex.slice(5, 7), 16) });

type Draft = { brightness: number; effect: KeyboardEffect | null; allColor: RgbColor; zones: KeyboardZones };
const emptyDraft = (): Draft => ({ brightness: 0, effect: null, allColor: { r: 0, g: 0, b: 0 },
  zones: [0, 1, 2].map(() => ({ enabled: true, color: { r: 0, g: 0, b: 0 } })) as KeyboardZones });
const fromProfile = (profile: KeyboardLightingProfile): Draft => profile.current ? {
  brightness: profile.current.enabled ? profile.current.brightnessLevel : 0, effect: profile.current.effect,
  allColor: profile.current.allColor, zones: profile.current.zones,
} : emptyDraft();
export function useKeyboardLighting(active: boolean, revision = 0) {
  const [profile, setProfile] = useState<KeyboardLightingProfile | null>(null), [draft, setDraft] = useState<Draft>(emptyDraft);
  const [error, setError] = useState<string | null>(null), [busy, setBusy] = useState(false);
  const [readError, setReadError] = useState<string | null>(null);
  const accepted = useRef<Draft | null>(null), running = useRef(false), generation = useRef(0);
  const reading = useRef<Promise<KeyboardLightingProfile> | null>(null);
  const load = useCallback(async (reset = false) => {
    if (reading.current || running.current && !reset) return;
    const id = ++generation.current, pending = readKeyboardLighting(); reading.current = pending;
    try {
      const next = await pending;
      if (id !== generation.current) return;
      const before = accepted.current, nextDraft = fromProfile(next); accepted.current = nextDraft;
      setProfile(next); setReadError(null);
      setDraft(previous => reset || before === null || JSON.stringify(previous) === JSON.stringify(before) ? nextDraft : previous);
    } catch (error) { if (id === generation.current) { setProfile(null); setReadError(String(error)); } throw error; }
    finally { if (reading.current === pending) reading.current = null; }
  }, []);
  useEffect(() => {
    let disposed = false;
    if (active) void (async () => { await reading.current?.catch(() => undefined); if (!disposed) await load(); })().catch(() => {});
    return () => { disposed = true; ++generation.current; };
  }, [active, revision, load]);
  const reload = () => { setError(null); void load().catch(() => {}); };
  const run = async (action: () => Promise<KeyboardApplyOutcome>) => {
    if (running.current) return;
    running.current = true; ++generation.current; setBusy(true); setError(null);
    try {
      await reading.current?.catch(() => undefined);
      const result = await action();
      if (!result.ok) throw new Error([result.message, result.rollbackError].filter(Boolean).join("；"));
      await load(true); toast({ kind: "success", title: "键盘灯设置已写入" });
    } catch (error) { setError(String(error)); }
    finally { running.current = false; setBusy(false); }
  };
  const family: LightingFamily | null = profile?.detectedFamily === "white" || profile?.detectedFamily === "zoneRgb" ? profile.detectedFamily : null;
  const request: KeyboardLightingRequest | null = family === "white" ? { family, brightness: draft.brightness }
    : family === "zoneRgb" && draft.effect ? { family, ...draft, effect: draft.effect } : null;
  const dirty = profile?.current != null && JSON.stringify(draft) !== JSON.stringify(fromProfile(profile));
  return { profile, family, ...draft, request, dirty, error: error ?? readError, busy, reload, run,
    setBrightness: (brightness: number) => setDraft(previous => ({ ...previous, brightness })),
    setEffect: (effect: KeyboardEffect) => setDraft(previous => ({ ...previous, effect })),
    setAllColor: (allColor: RgbColor) => setDraft(previous => ({ ...previous, allColor })),
    setZones: (zones: KeyboardZones) => setDraft(previous => ({ ...previous, zones })) };
}
