/**
 * CodeBlock — render a code string as real syntax-highlighted code through the
 * Pierre file viewer (not a diff). This is the one component every place code
 * appears routes through: a file's public surface is the class rendered here,
 * a database table is its DDL rendered here, a decision's change is the code
 * rendered here. The why/notes live inside the code as inline `//` comments,
 * authored in the model — never as boxed badge-rows beside the code.
 *
 * Pierre's worker pool is disabled (the only part that breaks under file://);
 * the shared highlighter is preloaded first so the block mounts populated.
 */
import { useEffect, useState } from "react";
import { File } from "@pierre/diffs/react";
import { ensureHighlighter, DIFF_THEME } from "../highlight";

const EXT: Record<string, string> = {
  php: "php",
  ts: "ts",
  typescript: "ts",
  tsx: "tsx",
  js: "ts",
  sql: "sql",
  json: "json",
};

function fileName(label: string | undefined, language: string): string {
  if (label && /\.[a-z]+$/i.test(label)) return label;
  const ext = EXT[language] ?? "txt";
  const base =
    label?.replace(/\W+/g, "-").toLowerCase().replace(/^-+|-+$/g, "") || "source";
  return `${base}.${ext}`;
}

export function CodeBlock({
  code,
  language,
  label,
}: {
  code: string;
  language: string;
  label?: string;
}) {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    let alive = true;
    ensureHighlighter().then(() => alive && setReady(true));
    return () => {
      alive = false;
    };
  }, []);

  return (
    <div className="codeblock">
      <div className="codeblock__body">
        {!ready ? (
          <div className="codeblock__loading">highlighting…</div>
        ) : (
          <File
            file={{ name: fileName(label, language), contents: code.trimEnd() }}
            options={{ theme: DIFF_THEME, disableFileHeader: true }}
            disableWorkerPool
          />
        )}
      </div>
    </div>
  );
}
