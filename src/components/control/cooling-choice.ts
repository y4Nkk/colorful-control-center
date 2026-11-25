import type { OemPerformanceChange, OemPerformanceOptions } from "../../api/client";

export type CoolingChoice = {
  change: OemPerformanceChange;
  confirm: boolean;
  fanTarget: "quiet" | "auto" | null;
};

type Option = "turboFan" | "dtt" | "quietPreference" | "quietMode";

export function optionChoice(options: OemPerformanceOptions, key: Option, enabled: boolean, custom: boolean): CoolingChoice {
  const quietFan = key === "quietMode" ? "current" : key === "quietPreference" ? enabled ? "entertainmentOn" : "entertainmentOff" : null;
  const quiet = quietFan !== null;
  const values = {
    turboFan: key === "turboFan" ? enabled : quiet && enabled && options.turboActive ? false : null,
    dtt: key === "dtt" ? enabled : null,
  };
  const changesTurbo = options.appliesInCurrentMode && values.turboFan !== null && values.turboFan !== options.turboActive;
  const fanTarget = quiet ? enabled ? "quiet" : "auto" : changesTurbo ? "auto" : null;
  return {
    change: { values, quietFan, replaceCustomFan: custom && fanTarget !== null },
    confirm: fanTarget !== null || options.currentMode === 3 && options.appliesInCurrentMode,
    fanTarget,
  };
}

export function choiceDisabled(options: OemPerformanceOptions, choice: CoolingChoice, custom: boolean, software: boolean) {
  const { values, quietFan } = choice.change;
  const applies = options.appliesInCurrentMode;
  if (options.blockedReason || options.currentMode !== 2 && options.currentMode !== 3) return true;
  if (values.turboFan !== null && options.current.turboFan === null || values.dtt !== null && options.current.dtt === null) return true;
  if (software && (applies || quietFan !== null)) return true;
  if (custom && options.currentMode === 2 && choice.fanTarget === "auto" && quietFan === null) return true;
  if (!applies || options.dttDriverAvailable === true || !(values.dtt ?? options.dttActive)) return false;
  return values.dtt !== null && (values.dtt !== options.current.dtt || values.dtt !== options.dttActive) ||
    values.turboFan !== null && (values.turboFan !== options.current.turboFan || values.turboFan !== options.turboActive);
}
