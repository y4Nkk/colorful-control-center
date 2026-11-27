import type { ProfileReference } from "../../app/useProfileReferences";
import { Loading } from "../Loading";

export function ProfileRuleImpact({ name, rules, operation, loading, error }: {
  name: string; rules: ProfileReference[]; operation: "replace" | "delete"; loading: boolean; error: string | null;
}) {
  const affected = rules.filter(rule => rule.name === name);
  if (loading) return <Loading label="正在读取关联规则…" />;
  if (!affected.length && !error) return null;
  return <div className="ccc-profile-rule-impact">
    {!!affected.length && <>
      <strong>{affected.map(rule => rule.label).join("、")}</strong>
      <p>{operation === "replace" ? "覆盖后，请到电源页重新确认这些规则。" : "删除后，请到电源页检查这些规则并重新选择档案。"}</p>
    </>}
    {error && <p>部分规则引用无法读取 · {error}</p>}
  </div>;
}
