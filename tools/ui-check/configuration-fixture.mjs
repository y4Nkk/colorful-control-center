export function installConfigurationFixture(supply) {
  const data = window.__uiFixture;
  data.supplyMode = supply;
  data.supplyModeRevision = 0;
  data.calibrationSupported = true;
  data.dust = { revision: "dust-0", running: false, current: { weekdays: 10, hour: 10, onResume: false }, initial: null,
    softwareActive: false, lastStart: null, startBlockedReason: null, scheduleBlockedReason: null };
  data.platform = { deepS5: true, warnings: [], powerDelivery: { revision: "pd-0", encoding: "adapterV1", currentLevel: 1, currentValid: true,
    availableLevels: [1, 2, 4], zeroBased: false, adapterWatts: 100, initial: null, restoreBlockedReason: null, blockedReason: null } };
  data.usb = { revision: "usb-0", recommendation: true, supported: true, activePlan: "balanced", activeValues: { ac: true, dc: true },
    plans: ["balanced", "highPerformance", "powerSaver"].map(id => ({ id, ac: false, dc: true })),
    missing: [], initial: [], blockedReason: null, restoreBlockedReason: null, warnings: [] };
  const string = text => ({ kind: 1, bytes: [...text, "\0"].flatMap(letter => [letter.charCodeAt(0), 0]) });
  data.lan = { revision: "lan-0", frame: { capabilities: { powerSaving: true, wakePatch: true }, recommendation: true,
    devices: [{ identity: { instance: "PCI\\ISOLATED\\1", driver: "{4d36e972-e325-11ce-bfc1-08002be10318}\\0001", netCfgId: string("isolated-1"), driverVersion: string("1"), matchingId: null },
      name: "隔离网卡", values: { powerSavingMode: string("0"), pnpCapabilities: null, wakeOnLink: null, s5WakeOnLan: null, shutdownWake: null, wakeOnSlot: string("1") } }] },
    initial: [], lastError: null, blockedReason: null };
  data.keymap = { revision: "keymap-0", configuration: { devices: [
    { device: "keyboard", enabled: true, bindings: [{ trigger: 30, action: { kind: "disable" } }] }, { device: "mouse", enabled: false, bindings: [] }] },
    enabledFlags: [1, 0], sourceName: "隔离保存配置", baselineName: null, canRestore: false, recoveryRequired: false,
    blockedReason: null, warnings: [], programsRequired: false, programs: { running: false, error: null } };
  data.profile.settings.powerDelivery = { level: 4, encoding: "adapterV1", adapterWatts: 100, availableLevels: [1, 2, 4] };
  data.profile.settings.usbPolicy = { plans: structuredClone(data.usb.plans) };
  data.profile.settings.display = { displays: [{ ...data.display.displays[0].current, frequency: 144 }] };
}
