// Mocked negative controls only; these tests never generate source evidence.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const mock = `#!/usr/bin/env node
const assert = require("node:assert/strict");
const fs = require("node:fs");
const args = process.argv.slice(2);
assert.equal(process.env.RUSTC_WRAPPER, "");
assert.equal(process.env.RUSTC_WORKSPACE_WRAPPER, "");
assert.equal(process.env.FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6, undefined);
assert.equal(args[args.indexOf("--features") + 1], "workgroup_reduce_u32");
assert.equal(args[args.indexOf("--bundle-version") + 1], "5");
const output = args[args.indexOf("--output") + 1];
const mode = process.env.LDS_EXPORT_CONTROL;
fs.writeFileSync(process.env.LDS_EXPORT_CONTROL_LOG, "mock invoked, not evidence\\n");
if (mode === "nonzero-partial") { fs.writeFileSync(output, "mock only"); process.exit(17); }
if (mode === "no-callback") process.exit(0);
if (mode === "invalid-utf8") { process.stderr.write(Buffer.from([255])); process.exit(0); }
if (mode === "excess-output") {
  process.stderr.write(Buffer.alloc(10 * 1024 * 1024, 65), () => process.exit(0));
} else {
console.error("mock authority false");
if (mode === "missing-bundle") process.exit(0);
if (mode === "symlink-bundle") fs.symlinkSync(process.env.LDS_EXPORT_CONTROL_LOG, output);
else throw new Error("unknown negative control");
}
`;

for (const mode of ["wrong-toolchain", "nonzero-partial", "no-callback", "invalid-utf8",
  "excess-output", "missing-bundle", "symlink-bundle"]) {
  test(`LDS source exporter refuses ${mode} without validated metadata`, () => {
    const scratch = mkdtempSync(join(tmpdir(), "fe2o3-lds-export-negative-"));
    try {
      const target = join(scratch, "mock-target");
      mkdirSync(join(target, "debug"), { recursive: true });
      writeFileSync(join(target, "debug/fe2o3-export-sim"), mock, { flag: "wx", mode: 0o700 });
      const rustc = join(scratch, "mock-rustc");
      const commit = mode === "wrong-toolchain" ? "0".repeat(40) : "55e86c996809902e8bbad512cfb4d2c18be446d9";
      writeFileSync(rustc, `#!/usr/bin/env node\nconsole.log("commit-hash: ${commit}\\nrelease: 1.96.0-nightly");\n`,
        { flag: "wx", mode: 0o700 });
      const output = join(scratch, "negative-output"), log = join(scratch, "mock.log");
      const result = spawnSync(process.execPath, [join(root, "scripts/resource-query-lds-source-export.mjs"), output], {
        cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target, RUSTC: rustc,
          LDS_EXPORT_CONTROL: mode, LDS_EXPORT_CONTROL_LOG: log,
          RUSTC_WRAPPER: "must-clear", RUSTC_WORKSPACE_WRAPPER: "must-clear",
          FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6: "must-clear" },
        encoding: "utf8", timeout: 15000, maxBuffer: 1024 * 1024,
      });
      assert.ifError(result.error);
      assert.notEqual(result.status, 0);
      assert.equal(result.signal, null);
      assert.equal(existsSync(log), mode !== "wrong-toolchain");
      assert.equal(existsSync(join(output, "export.json")), false);
      const failure = JSON.parse(readFileSync(join(output, "failure.json"), "utf8"));
      assert.equal(failure.synthetic_fallback, false);
      assert.equal(failure.hardware_observed, false);
      assert.equal(failure.grants_load_authority, false);
      assert.equal(failure.grants_launch_authority, false);
      if (mode === "wrong-toolchain") {
        assert.match(failure.error, /pinned nightly commit/u);
        assert.equal(existsSync(join(output, "export-process-observation.json")), false);
        return;
      }
      const observation = JSON.parse(readFileSync(join(output, "export-process-observation.json"), "utf8"));
      assert.equal(observation.observation_only, true);
      if (mode === "nonzero-partial") assert.equal(observation.exit_code, 17);
      if (mode === "excess-output") assert.match(failure.error, /8 MiB/u);
    } finally {
      // Only this test-created temporary directory; never retained source evidence.
      rmSync(scratch, { recursive: true, force: true });
    }
  });
}

test("LDS source exporter never overwrites an existing output directory", () => {
  const scratch = mkdtempSync(join(tmpdir(), "fe2o3-lds-export-existing-"));
  try {
    const target = join(scratch, "mock-target"), output = join(scratch, "existing");
    mkdirSync(join(target, "debug"), { recursive: true });
    mkdirSync(output);
    writeFileSync(join(target, "debug/fe2o3-export-sim"), mock, { mode: 0o700 });
    writeFileSync(join(output, "keep.txt"), "retained test data\n");
    const result = spawnSync(process.execPath, [join(root, "scripts/resource-query-lds-source-export.mjs"), output], {
      env: { ...process.env, CARGO_TARGET_DIR: target }, encoding: "utf8", timeout: 5000,
    });
    assert.ifError(result.error);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /EEXIST/u);
    assert.equal(readFileSync(join(output, "keep.txt"), "utf8"), "retained test data\n");
    assert.equal(existsSync(join(output, "failure.json")), false);
  } finally { rmSync(scratch, { recursive: true, force: true }); }
});
