export function installMultiZoneFixture() {
  const data = window.__uiFixture;
  const color = (r, g, b) => ({ r, g, b });
  data.family = "multiZone";
  data.keyboard.detectedType = 23;
  data.lightingOptions.capabilities.keyboardType = 23;
  data.multiZone = {
    revision: "multi-0", capabilities: { keyboardType: 23, lightBarSwitchable: true, lightBarEnabled: true, cover: true },
    current: { enabled: true, brightness: 4, mode: 1, speed: 3,
      zones: [color(20, 40, 220), color(40, 160, 80), color(220, 90, 30), color(150, 50, 170)],
      lightBar: color(30, 130, 180), cover: color(180, 180, 180) },
    initial: null, canRestore: false, unavailableReason: null, blockedReason: null,
  };
}
