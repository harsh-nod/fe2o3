#!/usr/bin/env node
// Public Cargo/source workflow. No private rustc callbacks, synthetic owners,
// native compiler worker, artifact finalizer, runtime, or GPU are invoked.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, constants, existsSync, fstatSync, lstatSync, mkdirSync, openSync, readdirSync, readSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const cases = {
  one: ["complete-body-one-v19", null],
  diamond: ["complete-body-diamond-v19", null],
  "dynamic-grid": ["complete-body-dynamic-grid-v19", "complete-body source requires an explicit finite max_grid"],
  "wrong-launch": ["complete-body-wrong-launch-v19", "complete body requires required and maximum 64x1x1"],
  "reserved-register": ["complete-body-reserved-register-v19", "complete body roles require distinct v8..v63 outside the reserved prefix"],
  "foreign-input": ["complete-body-foreign-input-v19", "complete body marker operands differ from exact root argument order"],
  "undefined-merge": ["complete-body-undefined-merge-v19", "OutputNotDefined { label: Gfx942CompleteBodyLabelV1(4) }"],
};
if (process.argv.length !== 4 || !Object.hasOwn(cases, process.argv[2])) {
  throw new Error("usage: node scripts/complete-body-source-v19.mjs one|diamond|wrong-launch|reserved-register|foreign-input|undefined-merge|dynamic-grid NEW_OUTPUT_DIRECTORY");
}
assert.equal(process.platform, "linux", "this bounded Cargo workflow requires Linux");
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = resolve(process.argv[3]);
const [feature, refusal] = cases[process.argv[2]];
const fixture = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
const rustc = process.env.RUSTC;
assert.ok(rustc && isAbsolute(rustc), "set RUSTC to the absolute pinned-nightly binary (no auto-install)");
const bin = resolve(process.env.FE2O3_COMPLETE_BODY_BIN_DIR_V19 ??
  join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug"));
const wrapper = join(bin, "fe2o3-rustc-extract");
mkdirSync(output); // Must be new. Never overwrite or reuse an earlier run.
const MiB = 1024 * 1024;
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx", mode: 0o600 });
const saveJson = (name, value) => save(name, JSON.stringify(value, null, 2) + "\n");
const observations = [];
const sources = [
  "Cargo.toml", "src/lib.rs", "src/complete_body_v19.rs",
].map(relative => join(root, fixture, relative));

function readBounded(path, cap) {
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const stat = fstatSync(fd);
    assert.ok(stat.isFile() && stat.size <= cap, "expected bounded regular file: " + path);
    const bytes = Buffer.alloc(stat.size + 1);
    let size = 0;
    while (size < bytes.length) {
      const count = readSync(fd, bytes, size, bytes.length - size, null);
      if (!count) break;
      size += count;
      assert.ok(size <= stat.size, "file grew during observation");
    }
    assert.equal(size, stat.size, "file shrank during observation");
    return bytes.subarray(0, size);
  } finally { closeSync(fd); }
}
function pin(path, cap = MiB) {
  const bytes = readBounded(path, cap);
  return { path, bytes: bytes.length, sha256: hash(bytes) };
}
// An observed-size stop bound, not an allocation reservation or whole-machine
// disk/RSS guarantee. Only this newly created output tree is inspected.
function observeFreshDisk() {
  const queue = [output];
  let size = 0, entries = 0;
  while (queue.length) {
    const directory = queue.pop();
    let children;
    try { children = readdirSync(directory); }
    catch (error) { if (error.code === "ENOENT") continue; throw error; }
    for (const child of children) {
      assert.ok(++entries <= 100000, "fresh output entry bound");
      const path = join(directory, child);
      let stat;
      try { stat = lstatSync(path); }
      catch (error) { if (error.code === "ENOENT") continue; throw error; }
      assert.ok(!stat.isSymbolicLink(), "fresh output must not contain symlinks");
      if (stat.isDirectory()) queue.push(path);
      else {
        assert.ok(stat.isFile(), "fresh output contains a non-regular entry");
        size += stat.size;
        assert.ok(size <= 1024 * MiB, "fresh output exceeds 1 GiB observed-size bound");
      }
    }
  }
  return size;
}
function sanitized() {
  const env = { ...process.env };
  for (const key of Object.keys(env)) {
    if (key.startsWith("FE2O3_") || key.startsWith("CARGO_TARGET_") && key.endsWith("_RUSTFLAGS")) delete env[key];
  }
  for (const key of ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET",
    "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"]) delete env[key];
  return env;
}
async function run(command, args, env) {
  return new Promise(resolveResult => {
    const child = spawn(command, args, { cwd: root, env, detached: true, stdio: ["ignore", "pipe", "pipe"] });
    const chunks = { stdout: [], stderr: [] };
    let bytes = 0, error = null;
    const stop = reason => {
      error ??= new Error(reason);
      if (child.pid !== undefined) {
        try { process.kill(-child.pid, "SIGKILL"); }
        catch (killError) { if (killError.code !== "ESRCH") error = killError; }
      }
    };
    const timeout = setTimeout(() => stop("child exceeded 300 seconds"), 300000);
    const disk = setInterval(() => { try { observeFreshDisk(); } catch (e) { stop(String(e)); } }, 500);
    for (const stream of ["stdout", "stderr"]) {
      child[stream].on("data", data => {
        if (error) return;
        bytes += data.length;
        if (bytes > 16 * MiB) stop("child output exceeded 16 MiB");
        else chunks[stream].push(data);
      });
    }
    child.on("error", value => { error ??= value; });
    child.on("close", (status, signal) => {
      clearTimeout(timeout); clearInterval(disk);
      resolveResult({ status, signal, error,
        stdout: Buffer.concat(chunks.stdout), stderr: Buffer.concat(chunks.stderr) });
    });
  });
}
try {
  const sourcePins = sources.map(path => pin(path));
  const binaryPins = [wrapper, join(bin, "librustc_codegen_fe2o3.so")].map(path => pin(path, 512 * MiB));
  const sysrootResult = await run(rustc, ["--print", "sysroot"], sanitized());
  assert.ifError(sysrootResult.error);
  assert.equal(sysrootResult.status, 0);
  const sysroot = new TextDecoder("utf-8", { fatal: true }).decode(sysrootResult.stdout).trimEnd();
  assert.ok(isAbsolute(sysroot) && sysroot.length <= 4096 && !/[\r\n]/u.test(sysroot));
  assert.ok(basename(sysroot).startsWith("nightly-2026-04-03-"), "wrong pinned compiler");
  saveJson("inputs.json", { schema: "fe2o3-complete-body-source-inputs-v19",
    feature, sourcePins, binaryPins, rustc, sysroot,
    compiler_closure_attestation: "unavailable", native_llvm_executed: false, hardware_observed: false });
  const env = { ...sanitized(),
    RUSTC: join(sysroot, "bin/rustc"),
    RUSTC_WRAPPER: "", CARGO_BUILD_RUSTC_WRAPPER: "",
    RUSTC_WORKSPACE_WRAPPER: wrapper, CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: wrapper,
    LD_LIBRARY_PATH: bin + ":" + join(sysroot, "lib"),
    FE2O3_EXTRACT_CRATE_V1: "fe2o3_production_extraction_fixture",
    CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS:
      "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on",
    CARGO_INCREMENTAL: "0", CARGO_BUILD_JOBS: "2", CARGO_PROFILE_RELEASE_DEBUG: "0",
  };
  for (const stage of ["llvm", "handoff"]) {
    const path = join(output, stage === "llvm" ? "canonical.ll" : "handoff-v2.bin");
    const envName = stage === "llvm" ? "FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1" :
      "FE2O3_EXTRACT_AMDGPU_COMPILER_HANDOFF_PATH_V1";
    const args = ["check", "--release", "--locked", "--offline", "-Zbuild-std=core", "--lib",
      "--manifest-path", join(root, fixture, "Cargo.toml"), "--no-default-features", "--features", feature,
      "--target", "amdgcn-amd-amdhsa", "--target-dir", join(output, stage + "-target")];
    const result = await run(join(sysroot, "bin/cargo"), args, { ...env, [envName]: path });
    save(stage + ".stdout", result.stdout); save(stage + ".stderr", result.stderr);
    observations.push({ stage, args, status: result.status, signal: result.signal,
      stdout_sha256: hash(result.stdout), stderr_sha256: hash(result.stderr) });
    saveJson(stage + "-process.json", observations.at(-1));
    assert.ifError(result.error);
    assert.equal(result.signal, null, "a signal is not an accepted rejection");
    const stderr = new TextDecoder("utf-8", { fatal: true }).decode(result.stderr);
    new TextDecoder("utf-8", { fatal: true }).decode(result.stdout);
    if (refusal) {
      assert.notEqual(result.status, 0, "invalid source was accepted");
      assert.ok(stderr.includes(refusal), "wrong rejection stage:\n" + stderr);
      assert.equal(existsSync(path), false, "refused source published an output");
      observations.at(-1).exact_source_refusal = refusal;
    } else {
      assert.equal(result.status, 0, stderr);
      assert.ok(stderr.includes("authenticated Rust -> MIR36 -> ranked/formal checked KIR19"));
      assert.ok(stderr.includes("artifact/launch authority false"));
      const bytes = readBounded(path, stage === "llvm" ? 16 * 1024 : MiB);
      assert.ok(bytes.length > 0);
      observations.at(-1).output = pin(path, MiB);
      if (stage === "llvm") {
        const llvm = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
        assert.ok(llvm.includes("asm sideeffect") && llvm.includes("v_mov_b32_e32"));
      } else {
        const llvm = readBounded(join(output, "canonical.ll"), 16 * 1024);
        const offset = bytes.indexOf(llvm);
        assert.ok(offset >= 0, "worker handoff does not retain the same executable LLVM");
        assert.equal(bytes.indexOf(llvm, offset + 1), -1, "ambiguous executable LLVM occurrence");
        assert.ok(bytes.subarray(offset + llvm.length).includes(Buffer.from(".fe2o3.kd.v1")));
        observations.at(-1).same_executable_llvm_text = true;
      }
    }
    observeFreshDisk();
    assert.deepEqual(sources.map(path => pin(path)), sourcePins);
    assert.deepEqual([wrapper, join(bin, "librustc_codegen_fe2o3.so")].map(path => pin(path, 512 * MiB)), binaryPins);
    saveJson(stage + "-validated.json", observations.at(-1));
  }
  saveJson("receipt.json", { schema: "fe2o3-complete-body-source-command-v19", feature, observations,
    sourcePins, binaryPins, source_unchanged: true, actual_cargo_rustc_wrapper: true,
    compiler_closure_attestation: "unavailable",
    exact_canonical_handoff_decode: "covered separately by Rust qualification ladder",
    cpu_simulation_run: false, formal_bounds_scope: "conservative LaunchEnvelope",
    protected_finalizer_admitted: false, native_llvm_executed: false, hardware_observed: false,
    grants_artifact_or_launch_authority: false });
  console.log("Complete-body source " + (refusal ? "exact refusal" : "LLVM and inert handoff") + " observed: " + output);
} catch (error) {
  saveJson("failure.json", { schema: "fe2o3-complete-body-source-command-failure-v19", error: String(error),
    feature, observations, synthetic_fallback: false, native_llvm_executed: false, hardware_observed: false });
  throw error;
}
