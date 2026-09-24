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
  one: ["physical-entry-one-v20", null],
  diamond: ["physical-entry-diamond-v20", null],
  registers: ["physical-entry-registers-v20", null],
  "wrong-launch": ["physical-entry-wrong-launch-v20", "physical-entry requires exact authored launch64 and max_grid2"],
  "foreign-input": ["physical-entry-foreign-input-v20", "physical-entry marker operands differ from exact root argument order"],
  "undefined-merge": ["physical-entry-undefined-merge-v20", "physical-entry reads an undefined physical register"],
  "missing-wait": ["physical-entry-missing-wait-v20", "physical load used before lgkm wait"],
  "wrong-carry": ["physical-entry-wrong-carry-v20", "physical pointer high origin"],
};
if (process.argv.length !== 4 || !Object.hasOwn(cases, process.argv[2])) {
  throw new Error("usage: node scripts/physical-entry-source-v20.mjs one|diamond|registers|wrong-launch|foreign-input|undefined-merge|missing-wait|wrong-carry NEW_OUTPUT_DIRECTORY");
}
assert.equal(process.platform, "linux", "this bounded Cargo workflow requires Linux");
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = resolve(process.argv[3]);
const [feature, refusal] = cases[process.argv[2]];
const fixture = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
const rustc = process.env.RUSTC;
assert.ok(rustc && isAbsolute(rustc), "set RUSTC to the absolute pinned-nightly binary (no auto-install)");
const bin = resolve(process.env.FE2O3_PHYSICAL_ENTRY_BIN_DIR_V20 ??
  join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug"));
const wrapper = join(bin, "fe2o3-rustc-extract");
mkdirSync(output); // Must be new. Never overwrite or reuse an earlier run.
const MiB = 1024 * 1024;
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx", mode: 0o600 });
const saveJson = (name, value) => save(name, JSON.stringify(value, null, 2) + "\n");
const observations = [];
const sources = [
  "Cargo.toml", "src/lib.rs", "src/physical_entry_v20.rs",
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
  saveJson("inputs.json", { schema: "fe2o3-physical-entry-source-inputs-v20",
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
  for (const stage of ["diagnostic"]) {
    const path = join(output, "diagnostic");
    const envName = "FE2O3_EXTRACT_DIAGNOSTIC_PHYSICAL_ENTRY_DIRECTORY_V20";
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
      assert.equal(existsSync(path), false, "refused source created diagnostic outputs");
      observations.at(-1).exact_source_refusal = refusal;
    } else {
      assert.equal(result.status, 0, stderr);
      assert.ok(stderr.includes("actual Rust -> MIR37 -> pre-ranked exact KIR20"));
      assert.ok(stderr.includes("ranked/formal/descriptor continuation unavailable"));
      assert.deepEqual(readdirSync(path).sort(), ["canonical-v20.bin", "canonical.ll", "native-observation-input-v20.txt"].sort());
      observations.at(-1).outputs = [
        ["canonical-v20.bin", MiB], ["canonical.ll", 64 * 1024], ["native-observation-input-v20.txt", 32 * 1024]
      ].map(([name, cap]) => pin(join(path,name),cap));
      const llvm = new TextDecoder("utf-8",{fatal:true}).decode(readBounded(join(path,"canonical.ll"),64*1024));
      const sidecar = new TextDecoder("utf-8",{fatal:true}).decode(readBounded(join(path,"native-observation-input-v20.txt"),32*1024));
      const canonical = readBounded(join(path,"canonical-v20.bin"),MiB);
      assert.ok(canonical.length>0 && llvm.includes("asm sideeffect") && llvm.includes("unreachable"));
      assert.ok(!llvm.includes("ret void"));
      assert.ok(sidecar.startsWith("FE2O3_PHYSICAL_ENTRY_V20_NATIVE_OBSERVATION_INPUT_V1\n"));
      assert.ok(sidecar.includes("llvm "+hash(Buffer.from(llvm))+" "+Buffer.byteLength(llvm)+"\n"));
      assert.ok(sidecar.endsWith("end\n"));
    }
    observeFreshDisk();
    assert.deepEqual(sources.map(path => pin(path)), sourcePins);
    assert.deepEqual([wrapper, join(bin, "librustc_codegen_fe2o3.so")].map(path => pin(path, 512 * MiB)), binaryPins);
    saveJson(stage + "-validated.json", observations.at(-1));
  }
  saveJson("receipt.json", { schema: "fe2o3-physical-entry-source-command-v20", feature, observations,
    sourcePins, binaryPins, source_unchanged: true, actual_cargo_rustc_wrapper: true,
    compiler_closure_attestation: "unavailable",
    normal_worker_handoff: false, ranked_formal_descriptor_continuation: "unavailable; pre-ranked diagnostic route",
    cpu_simulation_run: false, source_custody_exported: false,
    protected_finalizer_admitted: false, native_llvm_executed: false, hardware_observed: false,
    grants_artifact_or_launch_authority: false });
  console.log("Physical-entry pre-ranked source " + (refusal ? "exact refusal" : "inert KIR/LLVM/native-observation diagnostics") + " observed: " + output);
} catch (error) {
  saveJson("failure.json", { schema: "fe2o3-physical-entry-source-command-failure-v20", error: String(error),
    feature, observations, synthetic_fallback: false, native_llvm_executed: false, hardware_observed: false });
  throw error;
}
