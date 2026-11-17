export const ASPM_NAMES = ["关闭", "适度节能", "最大节能"];
export const PLAN_NAMES = ["平衡", "高性能", "节能"];
const NAMES: Record<string, string> = {
  "381b4222f69441f09685ff5bb260df2e": "平衡计划",
  "8c5e7fdae8bf4a969a85a6e23a8c635c": "高性能计划",
  "a184130835414fabbc81f71556f20b4a": "节能计划",
};
export const powerPlanName = (plan: string) => NAMES[plan] ?? `自定义计划 ${plan.slice(-4)}`;
