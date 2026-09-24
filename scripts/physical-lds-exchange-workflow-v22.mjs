// V22-only public Cargo wrapper. Files are observations, never compiler custody.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, constants, existsSync, fstatSync, lstatSync, mkdirSync,
  openSync, readdirSync, readSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { CASES_V22, caseV22, timelyV22, sourceRefusalV22, CONDITIONS_V22 } from "./physical-lds-exchange-contract-v22.mjs";
import { observedHandoffRelationV22 } from "./physical-lds-exchange-handoff-relation-v22.mjs";

const MiB = 1024 * 1024;
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const decode = bytes => new TextDecoder("utf-8", { fatal: true }).decode(bytes);
export async function runSourceWorkflowV22(mode) {
  assert.ok(["source", "checked"].includes(mode));
  assert.equal(process.platform, "linux", "bounded Cargo workflow requires Linux");
  if (process.argv.length !== 4 || !Object.hasOwn(CASES_V22, process.argv[2]))
    throw new Error("usage: node scripts/physical-lds-exchange-" + mode +
      "-v22.mjs " + Object.keys(CASES_V22).join("|") + " NEW_OUTPUT_DIRECTORY");
  const name = process.argv[2], { feature, refusal } = caseV22(name);
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const output = resolve(process.argv[3]);
  const rustc = process.env.RUSTC;
  assert.ok(rustc && isAbsolute(rustc), "set absolute pinned-nightly RUSTC; no auto-install");
  const bin = resolve(process.env.FE2O3_PHYSICAL_LDS_EXCHANGE_BIN_DIR_V22 ??
    join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug"));
  const wrapper = join(bin, "fe2o3-rustc-extract");
  const fixture = join(root, "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device");
  const sources = ["Cargo.toml", "src/lib.rs", "src/physical_lds_exchange_v22.rs"].map(p => join(fixture, p));
  const binaries = [wrapper, join(bin, "librustc_codegen_fe2o3.so")];
  const begun = performance.now(), timely = () => timelyV22(begun, performance.now(), 900000);
  mkdirSync(output, { mode: 0o700 }); // Fresh only. Never erase or overwrite old evidence.
  const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx", mode: 0o600 });
  const saveJson = (name, value) => save(name, JSON.stringify(value, null, 2) + "\n");
  const observations = [];

  function readBounded(path, cap) {
    const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
    try {
      const before = fstatSync(fd, { bigint: true });
      assert.ok(before.isFile() && before.size >= 0n && before.size <= BigInt(cap), "bounded regular file");
      const bytes = Buffer.alloc(Number(before.size));
      let at = 0;
      while (at < bytes.length) {
        const n = readSync(fd, bytes, at, bytes.length - at, at);
        assert.ok(n > 0, "file shrank during read"); at += n; timely();
      }
      assert.equal(readSync(fd, Buffer.alloc(1), 0, 1, at), 0, "file grew during read");
      const after = fstatSync(fd, { bigint: true }), current = lstatSync(path, { bigint: true });
      assert.ok(current.isFile() && !current.isSymbolicLink());
      for (const key of ["dev", "ino", "size", "mtimeNs", "ctimeNs"]) {
        assert.equal(before[key], after[key]); assert.equal(before[key], current[key]);
      }
      timely(); return bytes;
    } finally { closeSync(fd); }
  }
  function pin(path, cap = MiB) {
    const bytes = readBounded(path, cap);
    const result = { path, bytes: bytes.length, sha256: hash(bytes) };
    timely(); return result;
  }
  // Observed output-size stop bound, not allocation/RSS reservation or full sandbox.
  function freshDisk() {
    const queue = [output]; let size = 0, entries = 0;
    while (queue.length) {
      const directory = queue.pop();
      let children;
      try { children = readdirSync(directory); }
      catch (e) { if (e.code === "ENOENT") continue; throw e; }
      for (const child of children) {
        assert.ok(++entries <= 100000, "fresh output entry bound");
        const path = join(directory, child); let stat;
        try { stat = lstatSync(path); }
        catch (e) { if (e.code === "ENOENT") continue; throw e; }
        assert.ok(!stat.isSymbolicLink(), "fresh output symlink refused");
        if (stat.isDirectory()) queue.push(path);
        else {
          assert.ok(stat.isFile(), "non-regular fresh output");
          size += stat.size;
          assert.ok(size <= 1024 * MiB, "fresh output observed-size limit 1 GiB");
        }
        timely();
      }
    }
    timely(); return size;
  }
  function sanitized() {
    const env = { ...process.env };
    for (const key of Object.keys(env))
      if (key.startsWith("FE2O3_") || key.startsWith("CARGO_TARGET_") && key.endsWith("_RUSTFLAGS")) delete env[key];
    for (const key of ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET",
      "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"]) delete env[key];
    return env;
  }
  async function run(command, args, env) {
    timely();
    const started = performance.now();
    return await new Promise(resolveResult => {
      const child = spawn(command, args, { cwd: root, env, detached: true, stdio: ["ignore", "pipe", "pipe"] });
      const chunks = { stdout: [], stderr: [] };
      let bytes = 0, error = null, exited = false, closed = false, abandoned = false, drain;
      const boundDrain = () => {
        drain ??= setTimeout(() => {
          abandoned = true;
          error ??= new Error("child streams did not close within five-second drain bound");
          child.stdout.destroy(); child.stderr.destroy();
        }, 5000);
      };
      const stop = reason => {
        error ??= reason instanceof Error ? reason : new Error(String(reason));
        // Only the process group created by this launch, while its direct leader
        // is not observed exited. Never signal a number after losing that join.
        if (!exited && !closed && child.pid !== undefined) {
          try { process.kill(-child.pid, "SIGKILL"); }
          catch (e) { if (e.code !== "ESRCH") error = e; }
        }
        if (!closed) boundDrain();
      };
      const timeout = setTimeout(() => stop("child exceeded 300 seconds"), 300000);
      const disk = setInterval(() => { try { freshDisk(); } catch (e) { stop(e); } }, 500);
      child.once("error", stop);
      child.once("exit", () => { exited = true; boundDrain(); });
      for (const stream of ["stdout", "stderr"]) {
        child[stream].on("error", stop);
        child[stream].on("data", data => {
          if (error) return;
          bytes += data.length;
          if (bytes > 16 * MiB) stop("combined compiler streams exceeded 16 MiB");
          else chunks[stream].push(data);
        });
      }
      child.once("close", (status, signal) => {
        closed = true; clearTimeout(timeout); clearInterval(disk); if (drain) clearTimeout(drain);
        try { timelyV22(started, performance.now(), 300000); timely(); }
        catch (e) { error ??= e; }
        resolveResult({ status, signal, error, started, direct_child_exit_observed: exited,
          direct_child_close_observed: true, pipes_abandoned: abandoned, whole_family_cleanup_proved: false,
          stdout: Buffer.concat(chunks.stdout), stderr: Buffer.concat(chunks.stderr) });
      });
    });
  }
  function processAccepted(result) {
    timelyV22(result.started, performance.now(), 300000); timely();
    assert.ifError(result.error);
    assert.equal(result.pipes_abandoned, false);
    assert.equal(result.direct_child_exit_observed, true);
    assert.equal(result.direct_child_close_observed, true);
    assert.equal(result.signal, null);
  }

  try {
    const sourcePins = sources.map(path => pin(path));
    const binaryPins = binaries.map(path => pin(path, 512 * MiB));
    const sysrootResult = await run(rustc, ["--print", "sysroot"], sanitized());
    save("sysroot.stdout", sysrootResult.stdout); save("sysroot.stderr", sysrootResult.stderr);
    processAccepted(sysrootResult); assert.equal(sysrootResult.status, 0);
    const sysroot = decode(sysrootResult.stdout).trimEnd(); decode(sysrootResult.stderr);
    assert.ok(isAbsolute(sysroot) && sysroot.length <= 4096 && !/[\r\n]/u.test(sysroot));
    assert.ok(basename(sysroot).startsWith("nightly-2026-04-03-"), "wrong pinned compiler");
    processAccepted(sysrootResult);
    saveJson("inputs.json", { schema: "fe2o3-physical-lds-exchange-public-inputs-v22",
      mode, feature, sourcePins, binaryPins, rustc, sysroot,
      compiler_closure_attestation: "unavailable", native_llvm_executed: false, hardware_observed: false });
    const env = { ...sanitized(), RUSTC: join(sysroot, "bin/rustc"),
      RUSTC_WRAPPER: "", CARGO_BUILD_RUSTC_WRAPPER: "",
      RUSTC_WORKSPACE_WRAPPER: wrapper, CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: wrapper,
      LD_LIBRARY_PATH: bin + ":" + join(sysroot, "lib"),
      FE2O3_EXTRACT_CRATE_V1: "fe2o3_production_extraction_fixture",
      CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS:
        "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on",
      CARGO_INCREMENTAL: "0", CARGO_BUILD_JOBS: "2", CARGO_PROFILE_RELEASE_DEBUG: "0" };
    for (const stage of mode === "source" ? ["diagnostic"] : ["llvm", "handoff"]) {
      const path = join(output, stage === "diagnostic" ? "diagnostic" : stage === "llvm" ? "canonical.ll" : "handoff-v2.bin");
      const selector = stage === "diagnostic" ? "FE2O3_EXTRACT_DIAGNOSTIC_PHYSICAL_LDS_EXCHANGE_DIRECTORY_V22" :
        stage === "llvm" ? "FE2O3_EXTRACT_GFX942_LLVM_PATH_V1" : "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1";
      const args = ["check", "--release", "--locked", "--offline", "-Zbuild-std=core", "--lib",
        "--manifest-path", join(fixture, "Cargo.toml"), "--no-default-features", "--features", feature,
        "--target", "amdgcn-amd-amdhsa", "--target-dir", join(output, stage + "-target")];
      const result = await run(join(sysroot, "bin/cargo"), args, { ...env, [selector]: path });
      save(stage + ".stdout", result.stdout); save(stage + ".stderr", result.stderr);
      const row = { stage, args, status: result.status, signal: result.signal,
        stdout_sha256: hash(result.stdout), stderr_sha256: hash(result.stderr),
        direct_child_exit_observed: result.direct_child_exit_observed,
        direct_child_close_observed: result.direct_child_close_observed,
        pipes_abandoned: result.pipes_abandoned, whole_family_cleanup_proved: false };
      observations.push(row); saveJson(stage + "-process.json", row);
      processAccepted(result);
      const stderr = decode(result.stderr); decode(result.stdout); processAccepted(result);
      if (refusal) {
        row.exact_source_refusal = sourceRefusalV22(name, result.status, result.signal, stderr, existsSync(path));
      } else {
        assert.equal(result.status, 0, stderr);
        if (stage === "diagnostic") {
          assert.ok(stderr.includes("actual Rust -> MIR39 -> pre-ranked exact KIR22"));
          assert.ok(stderr.includes("ranked/formal/descriptor continuation unavailable"));
          assert.deepEqual(readdirSync(path).sort(), ["canonical-v22.bin", "canonical.ll", "native-observation-input-v22.txt"].sort());
          row.outputs = [["canonical-v22.bin", MiB], ["canonical.ll", 64 * 1024], ["native-observation-input-v22.txt", 32 * 1024]]
            .map(([name, cap]) => pin(join(path, name), cap));
          const canonical = readBounded(join(path, "canonical-v22.bin"), MiB);
          const llvmBytes = readBounded(join(path, "canonical.ll"), 64 * 1024), llvm = decode(llvmBytes);
          const sidecar = decode(readBounded(join(path, "native-observation-input-v22.txt"), 32 * 1024));
          assert.ok(canonical.length > 0);
          checkLLVM(llvm);
          assert.ok(sidecar.startsWith("FE2O3_PHYSICAL_LDS_EXCHANGE_V22_NATIVE_OBSERVATION_INPUT_V1\n"));
          assert.ok(sidecar.includes("llvm " + hash(llvmBytes) + " " + llvmBytes.length + "\n"));
          assert.ok(sidecar.includes("lds_frame 0 512 4 1\n") && sidecar.endsWith("end\n"));
        } else {
          assert.ok(stderr.includes("authenticated Rust -> MIR39 -> ranked/formal checked KIR22"));
          for (const condition of CONDITIONS_V22) assert.ok(stderr.includes(condition), condition);
          assert.ok(stderr.includes("artifact/launch authority false"));
          row.output = pin(path, stage === "llvm" ? 64 * 1024 : MiB);
          if (stage === "llvm") checkLLVM(decode(readBounded(path, 64 * 1024)));
          else {
            assert.ok(stderr.includes("protected/native/host admission unavailable"));
            row.diagnostic_relation = observedHandoffRelationV22(stderr,
              readBounded(join(output, "canonical.ll"), 64 * 1024), readBounded(path, MiB));
            saveJson("inert-handoff-relation.json", row.diagnostic_relation);
          }
        }
      }
      freshDisk();
      assert.deepEqual(sources.map(path => pin(path)), sourcePins);
      assert.deepEqual(binaries.map(path => pin(path, 512 * MiB)), binaryPins);
      processAccepted(result); saveJson(stage + "-validated.json", row); processAccepted(result);
    }
    assert.deepEqual(sources.map(path => pin(path)), sourcePins);
    assert.deepEqual(binaries.map(path => pin(path, 512 * MiB)), binaryPins);
    const receipt = { schema: "fe2o3-physical-lds-exchange-public-command-v22", mode, feature, observations,
      sourcePins, binaryPins, source_unchanged: true, actual_cargo_rustc_wrapper: true,
      compiler_closure_attestation: "unavailable",
      normal_worker_handoff: mode === "checked" && refusal === null,
      ranked_formal_descriptor_continuation: refusal ? "exact source refusal" :
        mode === "checked" ? "observed normal checked source to inert LLVM/handoff" : "not invoked by pre-ranked diagnostic endpoint",
      runtime_abi_conditions_discharged: false, source_custody_exported: false,
      diagnostic_relation_authenticates_compiler_or_source: false,
      cpu_simulation_run: false, lds_execution_observed: false,
      protected_finalizer_admitted: false, native_llvm_executed: false, hardware_observed: false,
      grants_artifact_or_launch_authority: false, whole_family_cleanup_proved: false,
      acceptance_requires_completed_exit_zero: true };
    timely(); saveJson("receipt.json", receipt); timely();
    await new Promise((resolveDone, reject) => process.stdout.write(
      "Physical LDS exchange " + mode + " " + (refusal ? "exact source refusal" : "inert output; runtime obligations unresolved") +
      " observed: " + output + "\n", e => e ? reject(e) : resolveDone()));
    timely();
  } catch (error) {
    saveJson("failure.json", { schema: "fe2o3-physical-lds-exchange-public-failure-v22", mode, feature,
      error: String(error).slice(0, 8192), observations, synthetic_fallback: false,
      any_receipt_json_alone_is_not_acceptance: true, native_llvm_executed: false, hardware_observed: false });
    throw error;
  }
}
function checkLLVM(llvm) {
  assert.ok(llvm.includes("asm sideeffect") && llvm.includes("unreachable") && !llvm.includes("ret void"));
  assert.ok(llvm.includes('"amdgpu-lds-size"="512,512"'));
}
