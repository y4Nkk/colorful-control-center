export const colorButton = (scope, label) => scope.getByRole("button", { name: label, exact: true });

export async function setPickerColor(scope, label, value) {
  await colorButton(scope, label).click();
  const picker = scope.getByRole("dialog", { name: label, exact: true });
  await picker.getByRole("textbox", { name: "HEX", exact: true }).fill(value);
  await colorButton(picker, "关闭选色").click();
}
