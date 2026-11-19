export function profileFiles(data) {
  return [...(data.profile ? [data.profile] : []), ...data.otherProfiles];
}

export async function invokeProfileStorage(data, command, args) {
  const files = profileFiles(data);
  if (command === "list_profiles") {
    if (data.profileListFault) throw new Error(data.profileListFault);
    return Promise.all(files.sort((a, b) => a.name.localeCompare(b.name)).map(async file => ({
      name: file.name, groups: profileGroups(file.settings), revision: await profileDigest(file),
    })));
  }
  if (command === "preview_profile_import") {
    const file = JSON.parse(args.json);
    if (file.version !== 1 || typeof file.name !== "string" || !file.name.trim() || !file.settings || !profileGroups(file.settings).length) {
      throw new Error("档案文件无效或没有设置");
    }
    return { ...file, settings: { ...emptyProfileSettings(), ...file.settings } };
  }
  if (command === "save_profile") {
    const { name, settings, expectedRevision } = args.args;
    const previous = files.find(file => file.name === name);
    const actual = previous ? await profileDigest(previous) : null;
    if (actual !== expectedRevision) return { ok: false, message: "档案已变化，请重新读取并确认" };
    data.otherProfiles = files.filter(file => file.name !== name);
    data.profile = { version: 1, name, savedAtMs: Date.now(), settings: structuredClone(settings) };
    return { ok: true, message: "档案已保存" };
  }
  const file = files.find(item => item.name === args.name);
  if (!file) throw new Error("档案不存在");
  if (command === "export_profile") return JSON.stringify(file);
  if (command === "load_profile") return structuredClone(file);
  if (command === "delete_profile") {
    if (await profileDigest(file) !== args.expectedRevision) return { ok: false, message: "档案已变化，请重新读取并确认" };
    if (data.profile === file) data.profile = data.otherProfiles.shift() ?? null;
    else data.otherProfiles = data.otherProfiles.filter(item => item !== file);
    if (data.startupSelection?.profile === file.name) data.startupSelection = null;
    return { ok: true, message: "档案已删除" };
  }
  throw new Error(`未声明的档案存储命令：${command}`);
}
