import { previewProfileAuthorization, backgroundProfileBinding } from "./mock-profile-authorization.mjs";

export function initPowerPolicy(data) {
  data.powerPolicy ??= { record: { version: 2, enabled: false, ac: null, battery: null, resume: null }, initial: null,
    paused: false, active: false, source: "ac", sleeping: false, pending: null,
    route: { modernStandby: false, displayEvents: false, warnings: [] }, configurationError: null, resetRevision: null, listenerError: null, lastIssue: null };
  return data.powerPolicy;
}

export async function savePowerPolicy(data, request) {
  const view = initPowerPolicy(data);
  if (JSON.stringify(request.expected) !== JSON.stringify(view.record)) throw new Error("自动档案预览已变化，请重新确认");
  if (view.configurationError) throw new Error(view.configurationError);
  if (request.enabled && (!view.route || view.listenerError)) throw new Error("电源通知尚未就绪");
  const target = { version: 2, enabled: request.enabled, ac: null, battery: null, resume: null };
  for (const key of ["ac", "battery", "resume"]) {
    const selected = request[key];
    if (!selected) continue;
    if (JSON.stringify(selected.file) !== JSON.stringify(data.profile)) throw new Error("自动档案内容已变化，请重新预览");
    const mode = selected.file.settings.oemPowerMode == null ? null : await previewProfileAuthorization(data, selected.file.settings);
    if (JSON.stringify(selected.mode) !== JSON.stringify(mode)) throw new Error("自动档案模式联动已变化，请重新预览");
    target[key] = await backgroundProfileBinding(selected.file, mode);
  }
  if (target.enabled && ![target.ac, target.battery, target.resume].some(Boolean)) throw new Error("至少选择一条规则");
  if (!data.powerPolicyHasBaseline) { view.initial = structuredClone(view.record); data.powerPolicyHasBaseline = true; }
  view.record = target; view.active = target.enabled; view.paused = false; view.lastIssue = null;
  return null;
}

export async function invokePowerPolicy(data, command, args) {
  const view = initPowerPolicy(data);
  if (command === "read_automatic_profile_application") {
    const report = data.profileApplication;
    if (report && ["ac", "battery", "resume"].includes(report.origin)) data.lastAutomaticProfileApplication = structuredClone(report);
    return structuredClone(data.lastAutomaticProfileApplication ?? null);
  }
  if (command === "read_power_profile_policy") {
    if (data.powerPolicyReadError) throw new Error(data.powerPolicyReadError);
    if (data.powerPolicyExternalRecord) { view.configurationError = "自动档案策略已被外部修改，请重新读取"; view.active = false; view.paused = true; }
    return structuredClone(view);
  }
  if (data.powerPolicyFault) { const error = data.powerPolicyFault; data.powerPolicyFault = null; throw new Error(error); }
  if (command === "save_power_profile_policy") return savePowerPolicy(data, args.request);
  if (command === "reset_power_profile_policy") {
    if (!view.configurationError || !view.resetRevision || args.expectedRevision !== view.resetRevision) throw new Error("不可用自动档案规则已变化，请重新读取后确认清除");
    Object.assign(view, { record: { version: 2, enabled: false, ac: null, battery: null, resume: null }, initial: null,
      configurationError: null, resetRevision: null, paused: true, active: false, lastIssue: null });
    return null;
  }
  if (command === "pause_power_profile_policy") {
    view.paused = true; view.active = false; view.pending = null;
    if (data.profileApplication?.pending && ["ac", "battery", "resume"].includes(data.profileApplication.origin)) {
      invokeProfileApplication(data, "cancel_profile_application", { id: data.profileApplication.id });
    }
    return null;
  }
  if (command === "reload_power_profile_policy") {
    if (view.configurationError && view.resetRevision && !data.powerPolicyExternalRecord) throw new Error(view.configurationError);
    if (data.powerPolicyExternalRecord) { view.record = structuredClone(data.powerPolicyExternalRecord); data.powerPolicyExternalRecord = null; }
    view.configurationError = null; view.lastIssue = null; view.paused = true; view.active = false; return null;
  }
  if (command === "restore_power_profile_policy") {
    if (view.configurationError) throw new Error(view.configurationError);
    if (JSON.stringify(args.expected) !== JSON.stringify(view.record) || !view.initial) throw new Error("初始规则预览已变化");
    view.record = view.initial; view.initial = null; view.lastIssue = null; view.paused = true; view.active = false; data.powerPolicyHasBaseline = false;
    return null;
  }
  throw new Error(`未声明的自动档案命令：${command}`);
}
