export async function invokePlatform(data, command, args) {
  const blocked = data.calibration.workerRunning || data.calibration.recoveryRequired;
  if (command === "read_windows_power_plans" || command === "apply_windows_power_plan") {
    const windowsPlans = data.windowsPlans ??= {
      activeId: "22222222-2222-2222-2222-222222222222",
      plans: [
        { id: "11111111-1111-1111-1111-111111111111", name: "卓越性能" },
        { id: "22222222-2222-2222-2222-222222222222", name: "高性能" },
        { id: "381b4222-f694-41f0-9685-ff5bb260df2e", name: "平衡" },
        { id: "55555555-5555-5555-5555-555555555555", name: "办公自定义" },
      ],
    };
    const { activeId, plans } = windowsPlans;
    const blockedReason = blocked ? "电池校准正在接管或尚需恢复，暂不能切换电源模式及其关联策略"
      : data.windowsPlansBlockedReason ?? null;
    const revision = JSON.stringify([activeId, plans, blockedReason]);
    if (command === "read_windows_power_plans") {
      if (data.windowsPlansReadFault) throw new Error(data.windowsPlansReadFault);
      return structuredClone({ activeId, plans, revision, blockedReason });
    }
    const { expectedRevision, targetId } = args.request;
    if (blockedReason) throw new Error(blockedReason);
    if (expectedRevision !== revision || !plans.some(plan => plan.id === targetId)) throw new Error("电源计划已变化，请重新选择");
    if (data.windowsPlansWriteFault) throw new Error(data.windowsPlansWriteFault);
    windowsPlans.activeId = targetId;
    return null;
  }

  const pd = data.platform.powerDelivery;
  switch (command) {
    case "read_platform_power":
      if (data.platformReadFault) throw new Error(data.platformReadFault);
      return structuredClone({ ...data.platform, powerDelivery: pd ? { ...pd, blockedReason: blocked ? "电池校准期间不能更改PD档位" : pd.blockedReason } : null });
    case "apply_pd_level": {
      if (!pd || blocked || args.request.expectedRevision !== pd.revision || !pd.availableLevels.includes(args.request.level)) throw new Error("PD预览已变化");
      pd.initial ??= { value: pd.currentLevel, zeroBased: pd.zeroBased };
      pd.currentLevel = args.request.level; pd.currentValid = true; pd.revision = `pd-${++data.pdRevision}`;
      if (data.pdFault) { const error = data.pdFault; data.pdFault = null; throw new Error(error); }
      return null;
    }
    case "restore_pd_level":
      if (!pd?.initial || blocked || pd.restoreBlockedReason || args.expectedRevision !== pd.revision) throw new Error("PD恢复预览已变化");
      pd.currentLevel = pd.initial.value; pd.currentValid = true; pd.revision = `pd-${++data.pdRevision}`; return null;
  }
}
