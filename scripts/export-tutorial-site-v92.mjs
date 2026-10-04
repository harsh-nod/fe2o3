#!/usr/bin/env node
// Run only against a clean, separately prepared site checkout. Never fetch or
// modify its source; Vite's cache belongs to the caller's private output lane.
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readFileSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";

const [siteArg, commit, manifestArg, cacheArg] = process.argv.slice(2);
if (!siteArg || !/^[a-f0-9]{40}$/.test(commit ?? "") || !manifestArg || !cacheArg
    || process.argv.length !== 6) throw Error("expected SITE COMMIT MANIFEST PRIVATE_CACHE");
const site = resolve(siteArg), manifestPath = resolve(manifestArg), cache = resolve(cacheArg);
function git(args) {
  const result = spawnSync("git", ["-C", site, ...args], { timeout: 10000,
    maxBuffer: 1024 * 1024, encoding: "utf8", env: { ...process.env,
      GIT_NO_LAZY_FETCH: "1", GIT_OPTIONAL_LOCKS: "0", GIT_TERMINAL_PROMPT: "0" } });
  if (result.error || result.status !== 0) throw Error(`site Git check refused: ${args[0]}`);
  return result.stdout.trim();
}
if (git(["rev-parse", "HEAD"]) !== commit) throw Error("site HEAD differs");
const tree = git(["rev-parse", "HEAD^{tree}"]);
git(["diff", "--quiet", "HEAD", "--"]);
const size = statSync(manifestPath).size;
if (!(size > 0 && size <= 4 * 1024 * 1024)) throw Error("manifest byte bound");
const bytes = readFileSync(manifestPath), sha256 = createHash("sha256").update(bytes).digest("hex");
if (bytes.length !== size) throw Error("manifest changed");
const require = createRequire(resolve(site, "package.json"));
const { createServer } = await import(require.resolve("vite"));
const vite = await createServer({ root: site, configFile: false, cacheDir: cache,
  appType: "custom", logLevel: "error", optimizeDeps: { noDiscovery: true },
  server: { middlewareMode: true, watch: null } });
try {
  const { lessons } = await vite.ssrLoadModule("/src/content/curriculum.ts");
  const { projectCurriculumTab, validateCurriculumEvidence } =
    await vite.ssrLoadModule("/scripts/curriculum-evidence.ts");
  const { authorFacingCode } = await vite.ssrLoadModule("/src/lib/kernel-authoring.ts");
  validateCurriculumEvidence(bytes, sha256, lessons);
  const inventory = { schema: "fe2o3-tutorial-runtime-projection-v1",
    site: { repository: "harsh-nod/fe2o3-kernels", commit, tree },
    lessons: lessons.map(lesson => ({ id: lesson.id, codeTabs: lesson.tabs.map((tab, ordinal) => ({
      ...projectCurriculumTab(tab, ordinal), displayedCode: authorFacingCode(tab).code,
      sourceFragments: tab.sourceFragments ?? null,
    })) })) };
  git(["diff", "--quiet", "HEAD", "--"]);
  if (git(["rev-parse", "HEAD"]) !== commit || !readFileSync(manifestPath).equals(bytes))
    throw Error("site or manifest changed");
  const output = JSON.stringify(inventory);
  if (Buffer.byteLength(output) > 16 * 1024 * 1024) throw Error("inventory byte bound");
  process.stdout.write(output + "\n");
} finally { await vite.close(); }
