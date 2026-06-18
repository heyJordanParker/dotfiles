/**
 * CodeDiff — the real before/after code change, never prose describing it.
 *
 * Rendered with @pierre/diffs (the named library): Shiki-based, so code
 * tokenizes by real grammar and never breaks into one-token-per-line. The
 * worker pool is disabled (it is the only part that breaks under file://); the
 * shared highlighter is preloaded on the main thread first, so the diff mounts
 * populated, not as an empty shell.
 *
 * Pierre renders into a custom element. Its rails are themed to OUR palette via
 * the `--diffs-*` custom properties set on `.codediff` in CSS — added rails read
 * the document's green, removed rails its red — so the diff speaks the same
 * one-color-one-meaning vocabulary as every badge and dot.
 *
 * A pure addition or deletion (one side only) renders that single version as a
 * plain Pierre File.
 */
import { useEffect, useState } from "react";
import { MultiFileDiff, File } from "@pierre/diffs/react";
import type { CodeChange } from "../model";
import { ensureHighlighter, DIFF_THEME } from "../highlight";

/** Map our short language ids to the filename extension Pierre infers from. */
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

export function CodeDiff({ change }: { change: CodeChange }) {
  const { language, before, after, layout = "split" } = change;
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let alive = true;
    ensureHighlighter().then(() => alive && setReady(true));
    return () => {
      alive = false;
    };
  }, []);

  const single = !before || !after;

  return (
    <div className="codediff" data-mode={single ? "single" : "diff"}>
      <div className="codediff__head">
        {single ? (
          <>
            <span
              className="codediff__state"
              data-state={after ? "added" : "removed"}
            >
              {after ? "new" : "deleted"}
            </span>
            <span className="codediff__path">
              {(after ? change.afterLabel : change.beforeLabel) ??
                fileName(undefined, language)}
            </span>
          </>
        ) : (
          <>
            <span className="codediff__state" data-state="removed">
              {change.beforeLabel ?? "before"}
            </span>
            <span className="codediff__arrow" aria-hidden="true">
              →
            </span>
            <span className="codediff__state" data-state="added">
              {change.afterLabel ?? "after"}
            </span>
          </>
        )}
      </div>

      <div className="codediff__body">
        {!ready ? (
          <div className="codediff__loading">highlighting…</div>
        ) : single ? (
          <File
            file={{
              name: fileName(
                after ? change.afterLabel : change.beforeLabel,
                language
              ),
              contents: (after ?? before ?? "").trimEnd(),
            }}
            options={{ theme: DIFF_THEME, disableFileHeader: true }}
            disableWorkerPool
          />
        ) : (
          <MultiFileDiff
            oldFile={{
              name: fileName(change.beforeLabel, language),
              contents: before!.trimEnd(),
            }}
            newFile={{
              name: fileName(change.afterLabel, language),
              contents: after!.trimEnd(),
            }}
            options={{
              theme: DIFF_THEME,
              diffStyle: layout,
              disableFileHeader: true,
            }}
            disableWorkerPool
          />
        )}
      </div>
    </div>
  );
}
