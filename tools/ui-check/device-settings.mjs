export const deviceRestoreButton = panel => panel.getByRole("button", { name: "恢复初始设置", exact: true });

export async function openDeviceRestore(panel) {
  await deviceRestoreButton(panel).click();
  return panel.getByRole("dialog", { name: "恢复初始设置", exact: true });
}

export async function restoreDevice(panel) {
  const dialog = await openDeviceRestore(panel);
  await dialog.getByRole("button", { name: "确认恢复", exact: true }).click();
}
