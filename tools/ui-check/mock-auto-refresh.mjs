export async function invokeAutoRefresh(data, command, args, emit) {
  const value = data.autoRefresh, current = data.display.displays[0].current;
  const snapshot = () => {
    const result = structuredClone(value);
    result.blockedReason ??= null;
    result.revision = `automatic-${data.autoRefreshRevision}`;
    result.current.display = structuredClone(current);
    result.canStart = !result.initial && result.listenerOnline && !result.error && !result.blockedReason;
    result.canRestore = !!result.initial;
    result.preview = result.initial ? null : { before: structuredClone(current), target: { ...current, frequency: result.current.savedOrHighest } };
    return result;
  };
  if (command === "read_automatic_refresh") return snapshot();
  if (args.expectedRevision !== `automatic-${data.autoRefreshRevision}`) throw new Error("自动刷新率预览已变化");
  if (command === "enable_automatic_refresh") {
    if (!snapshot().canStart) throw new Error("自动刷新率不可启用");
    value.initial = [...value.current.fields];
    value.initialDisplay = structuredClone(current);
    value.current.fields[0] = value.current.fields[1] = 1;
    value.active = true;
    data.autoRefreshRevision += 1;
    const target = { ...current, frequency: value.current.savedOrHighest };
    if (target.frequency !== current.frequency) {
      data.displayBefore = structuredClone(current);
      data.display.displays[0].current = structuredClone(target);
      data.display.pending = { before: [structuredClone(current)], targets: [target], rotation: false, automatic: true,
        trialId: (data.displayTrialId = (data.displayTrialId ?? 0) + 1), remainingMs: 15000, profileSession: null, error: null };
      data.displayExpiresAt = Date.now() + 15000;
      emit("automatic-display-trial", null);
    }
    return null;
  }
  if (command === "restore_automatic_refresh") {
    if (!value.initial) throw new Error("没有恢复记录");
    if (data.display.pending?.automatic) {
      data.display.displays[0].current = structuredClone(data.display.pending.before[0]);
      data.display.pending = null;
    } else if (data.autoRefreshOwned) {
      if (JSON.stringify(current) !== JSON.stringify(data.autoRefreshOwned)) throw new Error("主屏模式已变化，未覆盖外部设置");
      data.display.displays[0].current = structuredClone(value.initialDisplay);
    }
    value.current.fields = [...value.initial];
    value.initial = null; value.initialDisplay = null; value.active = false;
    data.autoRefreshOwned = null; data.autoRefreshRevision += 1;
    return null;
  }
}
