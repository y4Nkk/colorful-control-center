import { gpuTurboFixture } from "./mock-hotkeys.mjs";

export function modeGpuFixture() {
  const current = gpuTurboFixture().current;
  const raw = { devices: current.devices.map(device => device.id).concat(current.otherDevices), locks: structuredClone(current.locks), offsets: current.devices.map(device => structuredClone(device.offsets)) };
  const target = structuredClone(current.onTargets.find(target => target.mode === 3 && target.onAc).target);
  const projected = structuredClone(raw);
  if (target.lockMhz !== null) projected.locks.forEach((lock, index) => { lock.mode = index === 0 && target.lockMhz !== 0 ? 2 : 0; lock.value = index === 0 ? target.lockMhz * 1000 : 0; });
  target.offsets?.forEach((value, index) => { projected.offsets[index] = { coreKhz: value.coreMhz * 1000, memKhz: value.memoryMhz * 1000 }; });
  return { status: "ready", value: { nodeSupported: true, node: null, stages: null, action: { stage: "beforeMode", target: { enabled: true, automaticPowerOff: false } }, onAc: true,
    clocks: { policy: { capabilities: current.source.capabilities, limits: current.source.limits, saved: current.source.saved, gpuOc: true, simpleGpuOc: false, ready: true,
      gpuCount: raw.devices.length, cachedDeviceId: raw.devices[0].extended & 0xffff }, candidate: { decision: { target, skipped: null }, unavailableReason: null, projected },
      driver: { identity: "a".repeat(64), current: raw, devices: current.devices }, driverError: null, deferredReason: null } } };
}
