import { useEffect, useState } from "react";
import changelogUrl from "../../../CHANGELOG.md?url";
import { openExternalUrl, toCommandError } from "../../api/client";
import { Button } from "../Button";
import { Select } from "../Select";
import { toast } from "../use-toast";

const UNRELEASED = "未发布";

interface Entry {
  version: string;
  date?: string;
  notes: string;
}

type History = { status: "loading" } | { status: "failed" }
  | { status: "ready"; entries: Entry[]; Markdown: typeof import("react-markdown")["default"] };

function readEntries(source: string): Entry[] {
  const [, ...sections] = source.split(/^## /m);
  return sections.flatMap(section => {
    const boundary = section.indexOf("\n");
    if (boundary < 0) return [];
    const heading = /^\[(未发布|\d+\.\d+\.\d+)\](?: - (\d{4}-\d{2}-\d{2}))?$/.exec(section.slice(0, boundary).trim());
    const notes = section.slice(boundary + 1).trim();
    return heading && notes ? [{ version: heading[1], date: heading[2], notes }] : [];
  });
}

function versionLabel(version: string, appVersion?: string) {
  if (version === UNRELEASED) return UNRELEASED;
  return `v${version}${version === appVersion ? "（当前版本）" : ""}`;
}

async function loadEntries(signal: AbortSignal) {
  const response = await fetch(changelogUrl, { signal });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  return readEntries(await response.text());
}

export function ChangelogHistory({ appVersion }: { appVersion?: string }) {
  const [history, setHistory] = useState<History>({ status: "loading" });
  const [attempt, setAttempt] = useState(0);
  const [selected, setSelected] = useState<string>();

  useEffect(() => {
    const controller = new AbortController();
    void Promise.all([loadEntries(controller.signal), import("react-markdown")]).then(([entries, { default: Markdown }]) => {
      if (!controller.signal.aborted) setHistory({ status: "ready", entries, Markdown });
    }).catch(() => {
      if (!controller.signal.aborted) setHistory({ status: "failed" });
    });
    return () => controller.abort();
  }, [attempt]);

  if (history.status === "loading") return <p role="status">读取更新日志…</p>;
  if (history.status === "failed") return <div className="ccc-settings-feedback">
    <p className="ccc-settings-error" role="alert">读取更新日志失败。</p>
    <Button icon="refresh" size="compact" onClick={() => {
      setHistory({ status: "loading" });
      setAttempt(value => value + 1);
    }}>重试</Button>
  </div>;

  const entry = history.entries.find(item => item.version === selected)
    ?? history.entries.find(item => item.version === appVersion)
    ?? history.entries.find(item => item.version !== UNRELEASED)
    ?? history.entries[0];
  if (!entry) return <p>暂无更新记录。</p>;
  const { Markdown } = history;

  return <>
    <div className="ccc-changelog-version">
      <Select label="日志版本" value={entry.version} onChange={setSelected}
        options={history.entries.map(item => ({ value: item.version, label: versionLabel(item.version, appVersion) }))} />
      {entry.date && <time className="ccc-settings-detail" dateTime={entry.date}>{entry.date}</time>}
    </div>
    <article className="ccc-changelog-notes" aria-label={versionLabel(entry.version, appVersion)}>
      <Markdown skipHtml components={{
        a: ({ href, children }) => /^https?:\/\//i.test(href ?? "") && href
          ? <a href={href} onClick={event => {
            event.preventDefault();
            void openExternalUrl(href).catch(error => toast({ kind: "error", title: "无法打开链接", description: toCommandError(error).message }));
          }}>{children}</a>
          : <span>{children}</span>,
        img: ({ alt }) => alt ? <span>{alt}</span> : null,
      }}>{entry.notes}</Markdown>
    </article>
  </>;
}
