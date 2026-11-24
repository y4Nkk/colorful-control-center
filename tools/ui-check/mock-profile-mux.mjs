export function captureProfileGraphicsMode(data) {
  const before = data.graphicsMode;
  if (!before.current) return null;
  if (before.lastRequest?.verification === "uncertain") throw new Error("显卡选择中断，不能捕获未知选择");
  return { owner: data.graphicsOwner ?? "a".repeat(64), availableMask: before.current.availableMask,
    mode: before.lastRequest?.verification === "pendingReboot" ? before.lastRequest.target : before.current.mode };
}
export function previewProfileGraphicsMode(data, settings) {
  if (!settings.graphicsMode) return null;
  if (data.profileMuxPreviewFault) throw new Error(data.profileMuxPreviewFault);
  const current = captureProfileGraphicsMode(data), target = settings.graphicsMode;
  if (!current || current.owner !== target.owner || current.availableMask !== target.availableMask
    || target.mode < 1 || target.mode > 4 || !(current.availableMask & (1 << (target.mode - 1)))) throw new Error("档案显卡模式所属Windows或固件候选已变化");
  return { before: structuredClone(data.graphicsMode), target: target.mode, softwareFanRunning: data.fan.software !== null };
}
export function applyProfileGraphicsMode(data, settings) {
  const target = settings.graphicsMode;
  if (!target || captureProfileGraphicsMode(data).mode === target.mode) return;
  data.graphicsMode.lastRequest = { target: target.mode, verification: "pendingReboot" };
  data.graphicsMode.revision = `graphics-${++data.graphicsRevision}`;
  if (data.profileMuxAfterSendFault) throw new Error(data.profileMuxAfterSendFault);
}
export function restoreProfileGraphicsMode(data, before) {
  if (before.revision === data.graphicsMode.revision) return;
  const target = before.lastRequest?.verification === "pendingReboot" ? before.lastRequest.target : before.current.mode;
  data.graphicsMode.lastRequest = { target, verification: "pendingReboot" };
  data.graphicsMode.revision = `graphics-${++data.graphicsRevision}`;
  data.profileApplication.steps.push({ step: "恢复本次显卡选择（重启后核对）", ok: true, message: "已发送此前选择的恢复请求" });
}
