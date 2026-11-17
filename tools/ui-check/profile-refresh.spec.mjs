import { test } from "@playwright/test";
import { PROFILE_REFRESH_IPC, prepareProfileRefresh, refreshCases } from "./profile-refresh-cases.mjs";

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ content: PROFILE_REFRESH_IPC });
  await prepareProfileRefresh(page);
});
for (const item of refreshCases) test(item.name, async ({ page }) => item.run(page));
