import { syncProfileApplication } from "./mock-profile-application.mjs";

export async function invokeDisplay(data, command, args) {
  function snapshot() {
    const profile = structuredClone(data.display), primary = profile.displays.filter(item => item.primary);
    profile.blockedReason ??= null;
    profile.rotation = null;
    if (primary.length === 1 && [0, 1, 2, 3].includes(primary[0].current.orientation)) {
      const before = primary[0].current, target = structuredClone(before);
      if ([1, 3].includes(target.orientation)) [target.width, target.height] = [target.height, target.width];
      target.orientation = target.orientation === 2 ? 0 : 2;
      profile.rotation = { before, target };
    }
    return profile;
  }
  switch (command) {
    case "read_display_modes":
    case "read_automatic_display_trial": {
      const automaticOnly = command === "read_automatic_display_trial";
      const pending = data.display.pending;
      if (automaticOnly && !pending?.automatic) return null;
      if (pending?.profileSession != null) { syncProfileApplication(data); return snapshot(); }
      if (pending) {
        pending.remainingMs = Math.max(0, data.displayExpiresAt - Date.now());
        if (!pending.remainingMs) {
          data.display.displays[0].current = data.displayBefore;
          data.display.pending = null;
        }
      }
      if (automaticOnly) return structuredClone(data.display.pending);
      return snapshot();
    }
    case "apply_display_refresh":
      if (JSON.stringify(args.expected) !== JSON.stringify(data.display.displays[0].current)) throw new Error("显示预览已变化");
      data.displayBefore = structuredClone(args.expected);
      data.display.displays[0].current.frequency = args.frequency;
      data.display.pending = { before: [structuredClone(args.expected)], targets: [{ ...args.expected, frequency: args.frequency }], rotation: false, automatic: false, trialId: (data.displayTrialId = (data.displayTrialId ?? 0) + 1), remainingMs: 15000, profileSession: null, error: null };
      data.displayExpiresAt = Date.now() + 15000;
      return null;
    case "apply_display_rotation": {
      const display = data.display.displays.find(item => item.current.device === args.expected.device);
      if (!display?.primary || data.display.displays.filter(item => item.primary).length !== 1 || data.display.pending || JSON.stringify(args.expected) !== JSON.stringify(display.current)) throw new Error("主显示器预览已变化");
      data.displayBefore = structuredClone(args.expected);
      const target = structuredClone(args.expected);
      if (target.orientation === 1 || target.orientation === 3) [target.width, target.height] = [target.height, target.width];
      target.orientation = target.orientation === 2 ? 0 : 2;
      display.current = target;
      data.display.pending = { before: [structuredClone(args.expected)], targets: [target], rotation: true, automatic: false, trialId: (data.displayTrialId = (data.displayTrialId ?? 0) + 1), remainingMs: 15000, profileSession: null, error: null };
      data.displayExpiresAt = Date.now() + 15000;
      return null;
    }
    case "keep_display_mode":
      if (data.display.pending?.trialId !== args.trialId || Date.now() >= data.displayExpiresAt || JSON.stringify(args.expected) !== JSON.stringify(data.display.displays[0].current)) throw new Error("显示试用已变化或过期");
      if (data.display.pending.automatic) { data.autoRefreshOwned = structuredClone(args.expected); data.autoRefreshRevision += 1; }
      data.display.pending = null;
      return null;
    case "cancel_display_trial":
      if (!data.display.pending || data.display.pending.trialId !== args.trialId || data.display.pending.profileSession != null) throw new Error("没有可取消的显示试用");
      if (data.display.pending.automatic) data.autoRefreshRevision += 1;
      data.display.displays.find(item => item.current.device === data.displayBefore.device).current = structuredClone(data.displayBefore);
      data.display.pending = null;
      return null;
  }
}

