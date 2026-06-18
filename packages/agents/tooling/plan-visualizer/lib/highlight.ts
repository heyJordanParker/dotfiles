/**
 * Pierre highlighter preload — the one piece that makes @pierre/diffs render
 * code (not an empty shell) when the worker pool is disabled.
 *
 * Pierre highlights through a shared Shiki instance. With the worker pool on it
 * tokenizes off the main thread; with it OFF (our case — the worker is the only
 * thing that breaks under file://), pierre needs that shared instance already
 * loaded on the main thread, or it mounts the diff structure with no tokens and
 * zero height. So we preload the instance once, with the JS highlighter
 * (`shiki-js`, no wasm fetch — important offline), and every diff waits on the
 * same promise before mounting.
 */
import { preloadHighlighter } from "@pierre/diffs";

/** The languages any example diff might use. Loaded once, up front. */
const LANGS = ["php", "sql", "typescript", "tsx", "json"] as const;
const THEME = "github-dark";

let ready: Promise<void> | null = null;

export function ensureHighlighter(): Promise<void> {
  if (!ready) {
    ready = preloadHighlighter({
      themes: [THEME],
      // Pierre's SupportedLanguages is a broad union; these are all valid ids.
      langs: LANGS as unknown as Parameters<typeof preloadHighlighter>[0]["langs"],
      preferredHighlighter: "shiki-js",
    });
  }
  return ready;
}

export const DIFF_THEME = THEME;
