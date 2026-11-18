import { useLayoutEffect, useState, type ReactNode } from "react";
import { ConfigurationActions, createActionRegistry } from "../components/ConfigurationAction";
import { WorkspaceActivity } from "../components/Dialog";
import { ConfigurationReview } from "./ConfigurationReview";

export function ConfigurationPage({ active, children }: { active: boolean; children: ReactNode }) {
  const [registry] = useState(createActionRegistry);
  useLayoutEffect(() => {
    registry.setActive(active);
    return () => registry.setActive(false);
  }, [registry, active]);
  return <WorkspaceActivity.Provider value={active}>
    <ConfigurationActions.Provider value={registry}>
      <div hidden={!active} className="ccc-page">
        {children}
        <ConfigurationReview registry={registry} active={active} />
      </div>
    </ConfigurationActions.Provider>
  </WorkspaceActivity.Provider>;
}
