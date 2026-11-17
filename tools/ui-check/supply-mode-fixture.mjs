export function supplyModeFixture() {
  const ids = [{ pciDevice: 10336, subsystem: 407, revision: 1, extended: 10588, bus: 1, slot: 0 }];
  const raw = { devices: ids, locks: [0, 2].map((mode, domain) => ({ domain, mode, value: domain ? 1500000 : 0, unknown1: 1, unknown2: 2, unknown3: 3 })),
    offsets: [{ coreKhz: -12500, memKhz: 987654 }] };
  const target = mode => ({ mode, fromMode: null, gpu: null, revision: null, unavailableReason: null, acMode: null, energy: { hardwarePState: false, energyStar: [2, 1, 0, 2, 1][mode] }, turbo: mode === 2, dtt: mode === 2, fanMode: mode === 2 ? 6 : 0,
    fanSavedMode: 0, fanOffsetPct: mode === 2 ? 5 : 0, aspm: [1, 2], powerPlan: mode === 2 ? 1 : 0 });
  const conditions = ["ac", "battery", "startupAc", "startupBattery", "resumeAc", "resumeBattery"];
  const variants = Object.fromEntries(conditions.map(key => [key, { ...target(key.endsWith("Battery") || key === "battery" ? 1 : 2),
    powerPlan: ["ac", "battery"].includes(key) ? 0 : null }]));
  return { revision: "supply-0", enabled: false, canEnable: true, canRestore: false, recovery: null,
    error: null, recoveryError: null, listenerOnline: true, sleeping: false, active: false, canResume: false, issue: null, last: null,
    review: { deviceIdentity: "isolated-device", identity: "isolated-policy", revision: "isolated-review", currentMode: 3, acMode: 2,
      onAc: true, targets: [0, 1, 2, 3, 4].map(target), softwareFanActive: true, dcModePower: true, ...variants,
      currentEnergy: { hardwarePState: false, energyStar: 0 },
      gpu: { supported: true, current: { id: "isolated-node", name: "隔离独显", enabled: true, automaticPowerOff: false, problem: 0, drivesDisplay: false, canDisable: true },
        ...Object.fromEntries(conditions.map(key => [key, { action: key === "resumeBattery"
          ? { stage: "afterMode", target: { enabled: false, automaticPowerOff: false } }
          : { stage: "beforeMode", target: { enabled: true, automaticPowerOff: false } }, unavailableReason: null }])) },
      clocks: { policy: { capabilities: { limitClock: true, allModeOc: true }, limits: { quietMhz: 700, savingMhz: 800, performanceMhz: 1700, entertainmentMhz: 1400, thermalMhz: 0 },
        saved: [{ coreMhz: 123, memoryMhz: 456 }], gpuOc: true, simpleGpuOc: false, ready: true, gpuCount: 1, cachedDeviceId: 10588 },
        driver: { identity: "isolated-driver", current: raw, devices: ids.map(id => ({ id, offsets: raw.offsets[0], coreEditable: true, memoryEditable: true,
          coreRangeKhz: [-100000, 200000], memoryRangeKhz: [-200000, 1000000] })) }, driverError: null, deferredReason: null,
        ...Object.fromEntries(conditions.map(key => [key, { decision: key.startsWith("resume") ? { target: null, skipped: "resume" }
          : { target: { lockMhz: key === "ac" ? 1700 : 800, offsets: [{ coreMhz: 123, memoryMhz: 456 }] }, skipped: null },
          unavailableReason: null, projected: key.startsWith("resume") ? null : raw }])) },
    } };
}
export function supplyRecoveryFixture() {
  const fan = { software: null, package: null, rpmPairs: [{ query: 40, tables: [Array(112).fill(30), Array(112).fill(50)] }],
    fields: [{ offset: 5, data: [0] }, { offset: 7, data: [5] }], mode: 0, offset: 5 };
  const clocks = supplyModeFixture().review.clocks.driver.current;
  return { currentMode: 2, initialMode: 3, currentAcMode: null, initialAcMode: null, currentTurbo: true, initialTurbo: false, currentDtt: true, initialDtt: false,
    currentFan: fan, initialFan: { ...fan, fields: [{ offset: 5, data: [8] }, { offset: 7, data: [0] }], mode: 8, offset: 0 },
    currentFanMode: 0, initialFanMode: 8, currentFanOffset: 5, initialFanOffset: 0,
    currentSoftwareFan: false, initialSoftwareFan: false, fanSavedSettingsChanged: true,
    currentSystem: ["8c5e7fdae8bf4a969a85a6e23a8c635c", [0, 0]], initialSystem: ["381b4222f69441f09685ff5bb260df2e", [1, 2]],
    planChanges: [{ scheme: "a184130835414fabbc81f71556f20b4a", current: [0, 1], initial: [2, 2] }],
    currentEnergy: { hardwarePState: false, energyStar: 0 }, initialEnergy: { hardwarePState: false, energyStar: 2 },
    currentGpuPower: { id: "isolated-node", name: "隔离独显", target: { enabled: true, automaticPowerOff: false } },
    initialGpuPower: { id: "isolated-node", name: "隔离独显", target: { enabled: false, automaticPowerOff: true } },
    currentGpuClocks: null, initialGpuClocks: { origin: "afterEnable", state: clocks } };
}
