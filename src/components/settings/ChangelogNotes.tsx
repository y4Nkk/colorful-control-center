import Markdown from "react-markdown";
import { openExternalUrl, toCommandError } from "../../api/client";
import { toast } from "../use-toast";

export default function ChangelogNotes({ label, notes }: { label: string; notes: string }) {
  return <article className="ccc-changelog-notes" aria-label={label}>
    <Markdown skipHtml components={{
      a: ({ href, children }) => /^https?:\/\//i.test(href ?? "") && href
        ? <a href={href} onClick={event => {
          event.preventDefault();
          void openExternalUrl(href).catch(error => toast({ kind: "error", title: "无法打开链接", description: toCommandError(error).message }));
        }}>{children}</a>
        : <span>{children}</span>,
      img: ({ alt }) => alt ? <span>{alt}</span> : null,
    }}>{notes}</Markdown>
  </article>;
}
