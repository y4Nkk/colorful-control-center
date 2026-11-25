import { modeGpuFixture } from "./mode-gpu-fixture.mjs";
import { gpuTurboFixture } from "./mock-hotkeys.mjs";

export function modeCycleFixture(off = false) {
  const gpu = modeGpuFixture().value.clocks;
  const modes = [0, 1, 2, 3].map(mode => ({ mode, fromMode: null, gpu: null, revision: null, unavailableReason: null,
    acMode: mode, energy: { hardwarePState: false, energyStar: mode === 2 ? 0 : mode === 1 ? 1 : 2 },
    turbo: false, dtt: false, fan: { control: "explicit", mode: 0, savedMode: 0, offsetPct: 10 }, aspm: [1, 2], powerPlan: 0 }));
  const choices = gpuTurboFixture().current.onTargets.filter(choice => choice.mode !== 4).map(choice => {
    const projected = structuredClone(gpu.driver.current), target = choice.target;
    if (target.lockMhz !== null) projected.locks.forEach((lock, index) => { lock.mode = index === 0 && target.lockMhz !== 0 ? 2 : 0; lock.value = index === 0 ? target.lockMhz * 1000 : 0; });
    target.offsets?.forEach((value, index) => { projected.offsets[index] = { coreKhz: value.coreMhz * 1000, memKhz: value.memoryMhz * 1000 }; });
    return { mode: choice.mode, onAc: choice.onAc, candidate: { decision: { target, skipped: null }, projected: off ? null : projected, unavailableReason: null } };
  });
  return { modes, nodeSupported: true, node: { id: "ACPI\\NVPCF_FIXTURE", name: "隔离独显节点", enabled: !off,
    automaticPowerOff: true, problem: off ? 22 : 0, drivesDisplay: false, canDisable: true },
  nodeChoices: modes.map(({ mode }) => ({ mode, unavailableReason: null,
    action: { stage: mode === 2 || mode === 3 ? "beforeMode" : "afterMode", target: { enabled: mode !== 1, automaticPowerOff: true } } })),
  clocks: { policy: gpu.policy, choices, driver: off ? null : gpu.driver, driverError: null,
    deferredReason: off ? "独显已关闭；驱动范围和实际频率在已确认模式事务的写前检查点核对" : null } };
}
