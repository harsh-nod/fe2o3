// Negative script-control tests only. The child commands below are explicit
// mocks, never a compiler, and no successful compilation receipt is produced.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const script = join(root, "scripts/assembly-source-ranked-smoke.mjs");
const LLVM_CAP = 16 * 1024 * 1024;

const mockRustc = `#!/usr/bin/env node
if (process.argv.slice(2).join(" ") !== "--print sysroot") process.exit(93);
console.log(process.env.RANKED_SMOKE_CONTROL_SYSROOT);
`;

const mockCargo = `#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFileSync, closeSync, ftruncateSync, openSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const mode = process.env.RANKED_SMOKE_CONTROL_CASE;
const stage = process.env.FE2O3_EXTRACT_RANKED_MEMORY_V1 === "1" ? "ranked" : "production-target";
const targetIndex = process.argv.indexOf("--target-dir");
assert.ok(targetIndex >= 0);
const output = dirname(process.argv[targetIndex + 1]);
const llvm = join(output, "production-target.ll");
appendFileSync(process.env.RANKED_SMOKE_CONTROL_LOG, JSON.stringify({
  stage, mock_command: true,
  outer_wrapper: process.env.RUSTC_WRAPPER,
  outer_config_wrapper: process.env.CARGO_BUILD_RUSTC_WRAPPER,
  stale_extraction_flag: process.env.FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6 ?? null,
}) + "\\n");
if (stage === "ranked") {
  if (mode === "zero-exit-without-callback") process.exit(0);
  if (mode === "ranked-unexpected-llvm") writeFileSync(llvm, "mock unexpected output\\n");
  console.error('mock control: safety-verified lowering input for \x60assembly_chain\x60; all mandatory kernel checks clean true; bounds clean true; artifact/launch authority false\\nkernel.access mock-test-only');
  process.exit(0);
}
assert.equal(process.env.FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1, llvm);
if (mode === "nonzero-partial-llvm") {
  writeFileSync(llvm, "mock partial output, not LLVM evidence\\n");
  console.error("mock deliberate target rejection");
  process.exit(17);
}
console.error("mock control: ranked PLIRON -> Kernel IR V8 with 0 GuardedStore operation(s); composed formal/ranked memory -> target-KIR optimizer (7 pass(es)); artifact/launch authority false");
if (mode === "target-without-llvm") process.exit(0);
if (mode === "oversized-llvm") {
  const fd = openSync(llvm, "wx");
  try { ftruncateSync(fd, ${LLVM_CAP + 1}); } finally { closeSync(fd); }
} else if (mode === "symlink-llvm") {
  symlinkSync(process.env.RANKED_SMOKE_CONTROL_DATA, llvm);
} else if (mode === "fifo-llvm") {
  const result = spawnSync("mkfifo", [llvm], { timeout: 2000 });
  assert.ifError(result.error);
  assert.equal(result.status, 0);
} else {
  throw new Error("unknown negative control case");
}
`;

function runNegativeControl(mode, expectedStages, inspect) {
  const scratch = mkdtempSync(join(tmpdir(), "fe2o3-ranked-smoke-negative-"));
  try {
    const mockTarget = join(scratch, "mock-target");
    const mockBin = join(mockTarget, "debug");
    const sysroot = join(scratch, "mock-sysroot");
    mkdirSync(mockBin, { recursive: true });
    mkdirSync(join(sysroot, "lib"), { recursive: true });
    const wrapper = join(mockBin, "fe2o3-rustc-extract");
    const data = join(scratch, "mock-data.txt");
    writeFileSync(wrapper, "mock wrapper data only; never executed\n", { flag: "wx" });
    writeFileSync(data, "mock data only; not compiler evidence\n", { flag: "wx" });
    const rustc = join(scratch, "mock-rustc.mjs");
    const cargo = join(scratch, "mock-cargo.mjs");
    writeFileSync(rustc, mockRustc, { flag: "wx", mode: 0o700 });
    writeFileSync(cargo, mockCargo, { flag: "wx", mode: 0o700 });
    const log = join(scratch, "mock-invocations.jsonl");
    const output = join(scratch, "negative-output");
    const result = spawnSync(process.execPath, [script, output], {
      cwd: root,
      env: {
        ...process.env,
        RUSTC: rustc,
        CARGO: cargo,
        CARGO_TARGET_DIR: mockTarget,
        RUSTC_WRAPPER: "must-be-cleared-by-smoke",
        CARGO_BUILD_RUSTC_WRAPPER: "must-be-cleared-by-smoke",
        FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6: "must-be-cleared-by-smoke",
        RANKED_SMOKE_CONTROL_CASE: mode,
        RANKED_SMOKE_CONTROL_SYSROOT: sysroot,
        RANKED_SMOKE_CONTROL_LOG: log,
        RANKED_SMOKE_CONTROL_DATA: data,
      },
      encoding: "utf8",
      timeout: 15000,
      maxBuffer: 1024 * 1024,
    });
    assert.ifError(result.error);
    assert.notEqual(result.status, 0, "negative control unexpectedly succeeded");
    assert.equal(result.signal, null, "negative control must fail promptly, not time out");
    assert.equal(existsSync(join(output, "receipt.json")), false);
    assert.equal(existsSync(join(output, "production-target-validated-observation.json")), false);
    const input = JSON.parse(readFileSync(join(output, "input.json"), "utf8"));
    assert.equal(input.compiler_closure_attestation, "unavailable");
    assert.equal(input.hardware_observed, false);
    const failure = JSON.parse(readFileSync(join(output, "failure.json"), "utf8"));
    assert.equal(failure.status, "failed");
    assert.equal(failure.synthetic_fallback, false);
    assert.equal(failure.hardware_observed, false);
    assert.equal(failure.grants_artifact_authority, false);
    assert.equal(failure.grants_load_or_launch, false);
    const invocations = readFileSync(log, "utf8").trimEnd().split("\n").map(JSON.parse);
    assert.deepEqual(invocations.map(({ stage }) => stage), expectedStages);
    assert.deepEqual(failure.observations.map(({ stage }) => stage), expectedStages);
    for (const invocation of invocations) {
      assert.equal(invocation.mock_command, true);
      assert.equal(invocation.outer_wrapper, "");
      assert.equal(invocation.outer_config_wrapper, "");
      assert.equal(invocation.stale_extraction_flag, null);
    }
    inspect({ output, failure });
  } finally {
    // Only the exact directory just created by this test is removed. No retained
    // real compiler source, build output, or evidence capture is modified.
    rmSync(scratch, { recursive: true, force: true });
  }
}

test("zero exit without an extraction callback cannot pass ranked validation", () => {
  runNegativeControl("zero-exit-without-callback", ["ranked"], ({ output, failure }) => {
    assert.equal(failure.observations[0].exit_status, 0);
    assert.equal(failure.observations[0].llvm_output_present, false);
    assert.equal(existsSync(join(output, "ranked-validated-observation.json")), false);
    assert.match(failure.error, /launch authority false/u);
  });
});

test("ranked callback must not create a target LLVM output", () => {
  runNegativeControl("ranked-unexpected-llvm", ["ranked"], ({ output, failure }) => {
    assert.equal(failure.observations[0].exit_status, 0);
    assert.equal(failure.observations[0].llvm_output_present, true);
    assert.equal(existsSync(join(output, "ranked-validated-observation.json")), false);
  });
});

test("target callback text without an LLVM file cannot pass", () => {
  runNegativeControl("target-without-llvm", ["ranked", "production-target"], ({ output, failure }) => {
    assert.equal(existsSync(join(output, "ranked-validated-observation.json")), true);
    assert.equal(failure.observations[1].exit_status, 0);
    assert.equal(failure.observations[1].llvm_output_present, false);
    assert.match(failure.error, /ENOENT/u);
  });
});

test("a rejected target with partial output remains a failure observation", () => {
  runNegativeControl("nonzero-partial-llvm", ["ranked", "production-target"], ({ output, failure }) => {
    assert.equal(failure.observations[1].exit_status, 17);
    assert.equal(failure.observations[1].llvm_output_present, true);
    assert.equal(readFileSync(join(output, "production-target.ll"), "utf8"), "mock partial output, not LLVM evidence\n");
    assert.match(failure.error, /production-target rejected real source/u);
  });
});

for (const mode of ["oversized-llvm", "symlink-llvm", "fifo-llvm"]) {
  test(`${mode} fails a bounded regular-file read without blocking`, () => {
    runNegativeControl(mode, ["ranked", "production-target"], ({ output, failure }) => {
      assert.equal(failure.observations[1].exit_status, 0);
      assert.equal(failure.observations[1].llvm_output_present, true);
      const stat = lstatSync(join(output, "production-target.ll"));
      if (mode === "oversized-llvm") {
        assert.equal(stat.size, LLVM_CAP + 1);
        assert.match(failure.error, /bounded regular file/u);
      } else if (mode === "symlink-llvm") {
        assert.equal(stat.isSymbolicLink(), true);
        assert.match(failure.error, /ELOOP/u);
      } else {
        assert.equal(stat.isFIFO(), true);
        assert.match(failure.error, /bounded regular file/u);
      }
    });
  });
}
