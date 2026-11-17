export function captureProfileLan(data) {
  const frame = data.lan?.frame;
  if (!frame || !Object.values(frame.capabilities).some(Boolean)) return null;
  const string = value => value?.kind === 1 ? new TextDecoder("utf-16le", { fatal: true }).decode(new Uint8Array(value.bytes)).replace(/\0$/, "") : null;
  const devices = frame.devices.flatMap(device => {
    const values = {}, source = device.values;
    if (frame.capabilities.powerSaving && source.powerSavingMode !== null) {
      if (!["0", "1"].includes(string(source.powerSavingMode))) throw new Error("LAN节能字段未知，无法完整捕获");
      values.powerSavingMode = source.powerSavingMode;
    }
    if (frame.capabilities.wakePatch && ["s5WakeOnLan", "shutdownWake", "wakeOnSlot"].some(field => string(source[field]) !== null)) {
      values.pnpCapabilities = source.pnpCapabilities;
      if (string(source.wakeOnSlot) !== null) values.wakeOnLink = source.wakeOnLink;
    }
    return Object.keys(values).length ? [{ identity: device.identity, name: device.name, values }] : [];
  });
  return devices.length ? structuredClone({ capabilities: frame.capabilities, devices }) : null;
}
export function reviewProfileLan(data, target) {
  if (!target) return;
  if (!data.lan || JSON.stringify(target.capabilities) !== JSON.stringify(data.lan.frame.capabilities)) throw new Error("LAN档案能力已变化");
  for (const device of target.devices) {
    const current = data.lan.frame.devices.find(value => value.identity.instance === device.identity.instance);
    if (!current || JSON.stringify(current.identity) !== JSON.stringify(device.identity)) throw new Error("LAN档案驱动身份已变化");
  }
}
