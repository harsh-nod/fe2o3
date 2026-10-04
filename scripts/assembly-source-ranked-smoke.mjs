#!/usr/bin/env node
// Exercise real source-ranked and production target-lowering entry points.
// Never fall back to a bundle decoder, synthetic KIR, or an LLVM text emitter.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, constants, existsSync, fstatSync, mkdirSync, openSync, readSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.argv.length !== 3) {
  throw new Error("usage: node scripts/assembly-source-ranked-smoke.mjs NEW_OUTPUT_DIRECTORY");
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = resolve(process.argv[2]);
mkdirSync(output); // No overwrite or reuse of an earlier run's outputs.
const bin = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug");
const wrapper = join(bin, "fe2o3-rustc-extract");
const fixture = "crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30";
const sourcePath = `${fixture}/src/lib.rs`;
const source = readBoundedRegularFile(join(root, sourcePath), 1024 * 1024);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
const saveJson = (name, value) => save(name, `${JSON.stringify(value, null, 2)}\n`);
const observations = [];

function readBoundedRegularFile(path, cap) {
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const stat = fstatSync(fd);
    assert.ok(stat.isFile() && stat.size <= cap, "expected a bounded regular file");
    const buffer = Buffer.alloc(stat.size + 1);
    let count = 0;
    for (;;) {
      const read = readSync(fd, buffer, count, buffer.length - count, null);
      if (read === 0) break;
      count += read;
      assert.ok(count <= stat.size, "file grew during bounded observation");
    }
    assert.equal(count, stat.size, "file shrank during bounded observation");
    return buffer.subarray(0, count);
  } finally { closeSync(fd); }
}

// Linux process groups keep timeout/output bounds effective for Cargo's rustc
// descendants too. No shell, caller command string, or detached worker survives
// a timed-out or oversized capture.
async function runBounded(command, args, options = {}) {
  assert.equal(process.platform, "linux", "this compiler acceptance smoke requires Linux");
  return new Promise((resolveResult) => {
    const child = spawn(command, args, { cwd: root, env: options.env ?? process.env,
      detached: true, stdio: ["ignore", "pipe", "pipe"] });
    const streams = { stdout: [], stderr: [] };
    let bytes = 0;
    let error = null;
    const stop = (reason) => {
      error ??= new Error(reason);
      if (child.pid !== undefined) {
        try { process.kill(-child.pid, "SIGKILL"); }
        catch (killError) { if (killError.code !== "ESRCH") error = killError; }
      }
    };
    const timer = setTimeout(() => stop("compiler capture exceeded 300 seconds"), 300000);
    for (const name of ["stdout", "stderr"]) {
      child[name].on("data", (chunk) => {
        if (error) return;
        bytes += chunk.length;
        if (bytes > 16 * 1024 * 1024) stop("compiler capture exceeded 16 MiB");
        else streams[name].push(chunk);
      });
    }
    child.on("error", (spawnError) => { error ??= spawnError; });
    child.on("close", (status, signal) => {
      clearTimeout(timer);
      const stdoutBytes = Buffer.concat(streams.stdout);
      const stderrBytes = Buffer.concat(streams.stderr);
      resolveResult({ status, signal, error,
        stdoutBytes, stderrBytes,
        stdout: stdoutBytes.toString("utf8"), stderr: stderrBytes.toString("utf8") });
    });
  });
}

async function checked(command, args, options = {}) {
  const result = await runBounded(command, args, options);
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  return new TextDecoder("utf-8", { fatal: true }).decode(result.stdoutBytes);
}

try {
  const sysroot = (await checked(process.env.RUSTC ?? "rustc", ["--print", "sysroot"])).trimEnd();
  assert.ok(sysroot.length > 0 && sysroot.length <= 4096 && !/[\r\n]/u.test(sysroot));
  const env = { ...process.env };
  for (const key of Object.keys(env)) {
    if (key.startsWith("FE2O3_") || key.startsWith("CARGO_TARGET_") && key.endsWith("_RUSTFLAGS")) {
      delete env[key];
    }
  }
  for (const key of ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET",
    "RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"]) delete env[key];
  Object.assign(env, {
    RUSTC_WRAPPER: "",
    CARGO_BUILD_RUSTC_WRAPPER: "",
    RUSTC_WORKSPACE_WRAPPER: wrapper,
    CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: wrapper,
    LD_LIBRARY_PATH: `${bin}:${join(sysroot, "lib")}`,
    FE2O3_EXTRACT_CRATE_V1: "fe2o3_assembly_authoring_v30_fixture",
    CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS:
      "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
    CARGO_INCREMENTAL: "0",
  });
  saveJson("input.json", {
    source: sourcePath, source_bytes: source.length, source_sha256: hash(source),
    wrapper_sha256: hash(readBoundedRegularFile(wrapper, 512 * 1024 * 1024)),
    checkout_head: (await checked("git", ["rev-parse", "HEAD"])).trim(),
    checkout_dirty: (await checked("git", ["status", "--porcelain=v1"])).length !== 0,
    compiler_closure_attestation: "unavailable", hardware_observed: false,
  });
  for (const stage of ["ranked", "production-target"]) {
    const llvmPath = join(output, "production-target.ll");
    assert.equal(existsSync(llvmPath), false);
    const stageEnv = { ...env, ...(stage === "ranked"
      ? { FE2O3_EXTRACT_RANKED_MEMORY_V1: "1" }
      : { FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1: llvmPath }) };
    // Fresh targets force both rustc callbacks to execute, not reuse Cargo freshness.
    const result = await runBounded(process.env.CARGO ?? "cargo", [
      "check", "--release", "--locked", "--offline", "-Zbuild-std=core", "--lib",
      "--manifest-path", join(root, fixture, "Cargo.toml"),
      "--target", "amdgcn-amd-amdhsa", "--target-dir", join(output, `${stage}-extraction`),
    ], { env: stageEnv });
    const stderr = result.stderr ?? "";
    save(`${stage}.stdout`, result.stdoutBytes);
    save(`${stage}.stderr`, result.stderrBytes);
    observations.push({ stage, exit_status: result.status, signal: result.signal,
      stdout_sha256: hash(result.stdoutBytes), stderr_sha256: hash(result.stderrBytes),
      stdout_bytes: result.stdoutBytes.length, stderr_bytes: result.stderrBytes.length,
      llvm_output_present: existsSync(llvmPath) });
    saveJson(`${stage}-process-observation.json`, observations.at(-1));
    assert.ifError(result.error);
    new TextDecoder("utf-8", { fatal: true }).decode(result.stdoutBytes);
    new TextDecoder("utf-8", { fatal: true }).decode(result.stderrBytes);
    assert.equal(result.status, 0, `${stage} rejected real source:\n${stderr}`);
    assert.match(stderr, /artifact\/launch authority false/u);
    assert.doesNotMatch(stderr, /artifact\/launch authority true/u);
    if (stage === "ranked") {
      assert.equal(existsSync(llvmPath), false);
      assert.match(stderr, /safety-verified lowering input for `assembly_chain`/u);
      assert.match(stderr, /all mandatory kernel checks clean true/u);
      assert.match(stderr, /bounds clean true/u);
      assert.match(stderr, /kernel\.access /u);
    } else {
      // This unchanged source uses plain stores after explicit CFG bounds checks.
      // The existing production route therefore selects its canonical V8/seven-
      // pass profile, unlike the separately forced V6-bundle/KIR11 observation.
      assert.match(stderr, /ranked PLIRON -> Kernel IR V8 with 0 GuardedStore operation/u);
      assert.match(stderr, /composed formal\/ranked memory -> target-KIR optimizer \(7 pass\(es\)/u);
      const llvmBytes = readBoundedRegularFile(llvmPath, 16 * 1024 * 1024);
      const llvm = new TextDecoder("utf-8", { fatal: true }).decode(llvmBytes);
      assert.match(llvm, /target triple = "amdgcn-amd-amdhsa"/u);
      assert.match(llvm, /"target-cpu"="gfx942"/u);
      assert.match(llvm, /"target-features"="[^"]*-xnack/u);
      const assembly = [...llvm.matchAll(/call i32 asm sideeffect "([a-z0-9_]+) ([^"]+)", "([^"]+)"/gu)];
      assert.deepEqual(assembly.map((match) => match[1]).sort(), [
        "v_mov_b32", "v_mov_b32", "v_add_u32", "v_sub_u32", "v_xor_b32", "v_and_b32", "v_or_b32",
      ].sort());
      for (const match of assembly) {
        const unary = match[1] === "v_mov_b32";
        assert.equal(match[2], unary ? "$0, $1" : "$0, $1, $2");
        assert.equal(match[3], unary ? "=v,v" : "=v,v,v");
      }
      observations.at(-1).llvm_text_sha256 = hash(llvmBytes);
      observations.at(-1).llvm_text_bytes = llvmBytes.length;
      observations.at(-1).inline_assembly_count = assembly.length;
      observations.at(-1).production_kir_version = 8;
      observations.at(-1).production_optimizer_passes = 7;
    }
    saveJson(`${stage}-validated-observation.json`, observations.at(-1));
  }
  assert.equal(hash(readBoundedRegularFile(join(root, sourcePath), 1024 * 1024)), hash(source));
  saveJson("receipt.json", {
    schema: "fe2o3-assembly-source-ranked-smoke-v30", source: sourcePath,
    source_sha256: hash(source), observations,
    checks: ["actual_rustc_callbacks", "source_ranked_mandatory_checks", "production_target_lowering",
      "all_six_instruction_templates", "exact_vgpr_class_constraints", "unchanged_source"],
    compiler_closure_attestation: "unavailable", semantic_and_kir_identities: "not_exposed_by_this_diagnostic",
    observation_only: true, machine_code_generated: false, hardware_observed: false,
    grants_proof_authority: false, grants_artifact_authority: false, grants_load_or_launch: false,
  });
  console.log(`Real-source ranked and production target lowering passed; observations: ${output}`);
} catch (error) {
  saveJson("failure.json", { status: "failed", error: String(error), observations,
    synthetic_fallback: false, machine_code_generated: false, hardware_observed: false,
    grants_artifact_authority: false, grants_load_or_launch: false });
  throw error;
}
