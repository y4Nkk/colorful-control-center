import { BrandLogo } from "../BrandLogo";
import { Icon } from "../Icon";
import { Surface } from "../Surface";
import { Button } from "../Button";
import { useEffect, useId, useState } from "react";
import { getBackendInfo, toCommandError, type BackendInfo } from "../../api/client";

export function AboutSettings({ active }: { active: boolean }) {
  const id = useId();
  const [info, setInfo] = useState<BackendInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (!active) return;
    let current = true;
    setLoading(true);
    setError(null);
    void getBackendInfo().then((value) => {
      if (current) setInfo(value);
    }).catch((cause: unknown) => {
      if (current) setError(toCommandError(cause).message);
    }).finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [active, attempt]);
  return (
    <Surface as="section" variant="group" className="ccc-settings-group" aria-labelledby={`${id}-heading`}>
      <header className="ccc-settings-group-heading"><h2 className="ccc-icon-title" id={`${id}-heading`}><Icon name="info" />关于</h2></header>
      <div className="ccc-about-product">
        <BrandLogo className="ccc-about-logo" />
        <div className="ccc-about-copy"><h3>Colorful Control Center</h3></div>
        <span className="ccc-settings-status ccc-mono" role="status">{loading ? "读取版本…" : error ? "版本未知" : info ? `v${info.version}` : "版本未知"}</span>
      </div>
      {error && <div className="ccc-settings-feedback">
        <p className="ccc-settings-error" role="alert">版本读取失败：{error}</p>
        <Button icon="refresh" type="button" className="ccc-settings-retry" disabled={loading}
          onClick={() => setAttempt((value) => value + 1)}>重新读取版本</Button>
      </div>}
      <p className="ccc-settings-detail">社区项目 · 非官方</p>
    </Surface>
  );
}
