export function installTouchpadFixture() {
  const data = window.__uiFixture;
  data.touchpad = { revision: "touchpad-0", current: { devices: ["ACPI\\I2C0001\\0"], enabled: true, saved: 0 }, blockedReason: null };
  data.profile.settings.touchpad = { devices: ["ACPI\\I2C0001\\0"], enabled: false, savedEnabled: true };
}
