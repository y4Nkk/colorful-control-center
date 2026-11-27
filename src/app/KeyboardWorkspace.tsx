import { useState } from "react";
import { KeyboardLighting } from "../components/control/KeyboardLighting";
import { MultiZoneLighting } from "../components/control/MultiZoneLighting";
import { PerkeyLighting } from "../components/control/PerkeyLighting";
import { LightingOptions } from "../components/control/LightingOptions";
import { LightBarSettings } from "../components/control/LightBarSettings";
import { KeymapSettings } from "../components/keymap/KeymapSettings";
import { HotkeySettings } from "../components/control/HotkeySettings";
import { useKeyboardLighting } from "../components/control/useKeyboardLighting";
import { useMultiZoneLighting } from "../components/control/useMultiZoneLighting";
import { usePerkeyLighting } from "../components/control/usePerkeyLighting";
import { useLightingOptions } from "../components/control/useLightingOptions";
import { useLightBarSettings } from "../components/control/useLightBarSettings";
import { useKeymap } from "../components/keymap/useKeymap";
import { useHotkeySettings } from "../components/control/useHotkeySettings";
import type { HotkeyNavigationRequest } from "../api/client";
import { pageTitle } from "./navigation";
import { useHotkeyNavigation } from "./useHotkeyNavigation";
import type { HardwareWorkspaceProps } from "./useControlModel";
import { Loading } from "../components/Loading";

type Props = HardwareWorkspaceProps & { navigationRequest: HotkeyNavigationRequest | null; onNavigated: (id: number) => Promise<boolean> };
export function KeyboardWorkspace({ active, model, navigationRequest, onNavigated }: Props) {
  const [shown, setShown] = useState(false);
  const revision = model.deviceRevision;
  const keyboard = useKeyboardLighting(active, revision);
  const family = keyboard.profile?.detectedFamily;
  const perkey = usePerkeyLighting(active && family === "perkey", revision);
  const multiZone = useMultiZoneLighting(active, revision);
  const keymap = useKeymap(active, revision);
  const hotkeys = useHotkeySettings(active, revision);
  const options = useLightingOptions(active, revision);
  const lightBar = useLightBarSettings(active, revision);

  const ready = shown || (keyboard.loaded && (family !== "perkey" || perkey.loaded)
    && multiZone.loaded && keymap.loaded && hotkeys.loaded && options.loaded && lightBar.loaded);
  if (ready && !shown) setShown(true);
  const anchor = useHotkeyNavigation(active && ready, navigationRequest, onNavigated, "keyboard");

  if (!ready) return <Loading label={`正在加载${pageTitle("keyboard")}…`} />;
  return <div className="ccc-control">
    <div ref={anchor} role="region" aria-label="键盘灯光区域" tabIndex={-1} className="ccc-control">
      {family === "perkey" ? <PerkeyLighting model={perkey} /> : family !== "multiZone" && <KeyboardLighting state={keyboard} />}
      <MultiZoneLighting model={multiZone} />
    </div>
    <div className="ccc-module-grid" data-module-layout="adaptive">
      <KeymapSettings active={active} model={keymap} />
      <HotkeySettings model={hotkeys} />
    </div>
    <LightingOptions model={options} />
    <LightBarSettings model={lightBar} />
  </div>;
}
