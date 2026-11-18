import { openConfigurationReview } from "./configuration-review.mjs";

export const profilePanel = page => page.getByRole("region", { name: "档案预览", exact: true });

export async function openProfileDetails(page) {
  await profilePanel(page).getByRole("button", { name: "详情", exact: true }).click();
  return page.getByRole("dialog", { name: "档案详细信息", exact: true });
}

export async function openProfileResults(page) {
  await profilePanel(page).getByRole("button", { name: /^执行明细/ }).click();
  return page.getByRole("dialog", { name: /^执行明细/ }).getByLabel("执行结果");
}

export async function openStartupParameters(page) {
  const review = await openConfigurationReview(page, "启动档案");
  await review.getByRole("button", { name: "启动档案参数", exact: true }).click();
  return page.getByRole("dialog", { name: "启动档案参数", exact: true });
}
