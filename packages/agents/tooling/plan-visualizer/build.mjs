/**
 * The one builder. Turns a proposal model into a self-contained, double-click
 * .html review — all JS and CSS inlined, renders under file:// with no server.
 *
 *   node build.mjs <model> [outDir]
 *
 *   <model>     path to a .ts file that default-exports a ReviewModel (or
 *               named-exports exactly one ReviewModel). Per-run input — author
 *               it under /tmp, never inside this tooling directory.
 *   [outDir]    where the .html lands. Defaults to the model's own directory.
 *               The file is written as <outDir>/<name>.html, where <name> is the
 *               model file's basename.
 *
 * Every per-run artifact — the authored model, the build scratch, the output
 * HTML — lives under /tmp, never in this tooling directory. The directory ships
 * only managed code: the library, this builder, and the installed dependencies.
 * Putting per-run I/O under /tmp also keeps the agent runnable while the session
 * is mid-proposal, where the proposing guard permits /tmp writes but blocks
 * writes inside the working directory.
 *
 * Each run is its own single-input build, because the single-file plugin needs
 * inlineDynamicImports and rollup forbids that with multiple inputs. The entry
 * and HTML shell are generated into a /tmp scratch dir and torn down after, so a
 * model is the only thing an author writes — never the React boilerplate.
 * Concurrent runs on different models never collide: each uses its own scratch
 * dir, keyed by the model's basename.
 */
import { build } from "vite";
import react from "@vitejs/plugin-react";
import { viteSingleFile } from "vite-plugin-singlefile";
import { fileURLToPath, URL } from "node:url";
import { rm, mkdir, writeFile, copyFile, access, symlink, realpath } from "node:fs/promises";
import { resolve, dirname, basename } from "node:path";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

const modelArg = process.argv[2];
if (!modelArg) {
  console.error("usage: node build.mjs <model> [outDir]");
  process.exit(1);
}

// Resolve the model path against the caller's cwd, so a relative path works from
// anywhere. The basename (minus .ts) names the output file and scratch dir.
const modelFile = resolve(process.cwd(), modelArg);
try {
  await access(modelFile);
} catch {
  console.error(`no such model: ${modelArg}`);
  process.exit(1);
}
const name = basename(modelFile).replace(/\.ts$/, "");
const outDir = process.argv[3] ?? dirname(modelFile);

// All per-run scratch lives under /tmp — not in this managed tooling directory.
// /tmp is also what the session's proposing guard whitelists, so building a
// review while mid-proposal is not blocked. Resolve /tmp to its real path
// (/private/tmp on macOS) up front: Vite realpaths `root` internally, and if the
// input HTML path still carried the /tmp symlink the two would diverge and Rollup
// would emit the output under a "../" path. The guard whitelists /private/tmp too.
const tmpBase = await realpath("/tmp");
const tmp = `${tmpBase}/plan-visualizer-build/${name}`;
await rm(tmp, { recursive: true, force: true });
await mkdir(tmp, { recursive: true });

// Vite resolves bare imports (react, the vite plugins, @pierre/diffs) — including
// the react/jsx-runtime that plugin-react injects synthetically — from `root`
// upward. With root under /tmp that misses the tooling's deps, so symlink the
// installed node_modules into the scratch dir. The link lives under /tmp; the
// tooling's node_modules is read, never written.
await symlink(here("./node_modules"), `${tmp}/node_modules`, "dir");

const entryFile = `${tmp}/entry.tsx`;
const htmlFile = `${tmp}/index.html`;
// Import the whole module namespace and pick the one ReviewModel-shaped export
// at runtime. This means the author exports their model however reads cleanest —
// `export const fooModel` or `export default` — and never has to match a name
// the builder hard-codes. (A static `import foo from` would fail at bundle time
// when the model has no default export.) The model is imported by absolute path,
// so it resolves wherever the author wrote it (under /tmp); its own `@lib`
// imports resolve through the alias below to this tooling's lib/.
await writeFile(
  entryFile,
  `import { mount } from "${here("./example/mount")}";\n` +
    `import * as proposalModule from "${modelFile}";\n` +
    `const model = Object.values(proposalModule).find(\n` +
    `  (value) => value && typeof value === "object" && "problems" in value && "nodes" in value\n` +
    `);\n` +
    `if (!model) throw new Error("${modelFile} exports no ReviewModel (need an object with 'problems' and 'nodes')");\n` +
    `mount(model);\n`
);
await writeFile(
  htmlFile,
  `<!doctype html>\n<html lang="en">\n  <head>\n    <meta charset="UTF-8" />\n` +
    `    <meta name="viewport" content="width=device-width, initial-scale=1.0" />\n` +
    // Placeholder only — shown for the instant before the bundle mounts, then
    // replaced by the model's own `title` (see example/mount.tsx). The shell is
    // written before the model is loaded, so the real title can't be set here.
    `    <title>architecture review</title>\n  </head>\n` +
    `  <body>\n    <div id="root"></div>\n` +
    `    <script type="module" src="./entry.tsx"></script>\n  </body>\n</html>\n`
);

// root = the /tmp scratch dir, so the input HTML sits at the build root and the
// single-file plugin emits one flat dist/index.html (Vite mirrors the input's
// path relative to root into the output, so a nested input would nest the
// output). The entry imports the library and the model by absolute path; the
// @lib alias points back into this tooling dir; bare deps resolve through the
// node_modules symlinked into the scratch dir above. dedupe keeps one React copy.
const buildOut = `${tmp}/dist`;
await build({
  configFile: false,
  root: tmp,
  plugins: [react(), viteSingleFile()],
  resolve: { alias: { "@lib": here("./lib") }, dedupe: ["react", "react-dom"] },
  logLevel: "warn",
  build: {
    outDir: buildOut,
    emptyOutDir: true,
    assetsInlineLimit: Infinity,
    cssCodeSplit: false,
    rollupOptions: { input: htmlFile },
  },
});

// singlefile inlines everything; the one emitted .html is the deliverable.
await mkdir(outDir, { recursive: true });
const dest = `${outDir}/${name}.html`;
await copyFile(`${buildOut}/index.html`, dest);
await rm(tmp, { recursive: true, force: true });

console.log(`built → ${dest}`);
