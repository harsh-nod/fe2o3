#!/usr/bin/env node
// Export the actual existing Rust LDS fixture. No replacement program or
// fallback bundle; this records an observation, not compiler authentication.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { decodeUtf8, LIMITS, readBounded, sha256, SOURCE_PATH, validateExport }
  from "./resource-query-lds-v5-smoke.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

async function runBounded(program, args, env) {
  assert.equal(process.platform, "linux", "bounded compiler process groups require Linux");
  return new Promise((resolveResult) => {
    const child = spawn(program, args, { cwd: root, env, detached: true,
      stdio: ["ignore", "pipe", "pipe"] });
    const chunks = { stdout: [], stderr: [] };
    let bytes = 0, error;
    const stop = (reason) => {
      error ??= new Error(reason);
      if (child.pid !== undefined) try { process.kill(-child.pid, "SIGKILL"); }
      catch (killError) { if (killError.code !== "ESRCH") error = killError; }
    };
    const timer = setTimeout(() => stop("source export exceeded 300 seconds"), 300000);
    for (const name of ["stdout", "stderr"]) child[name].on("data", (chunk) => {
      if (error) return;
      bytes += chunk.length;
      if (bytes > LIMITS.export_log_bytes) stop("source export exceeded 8 MiB output");
      else chunks[name].push(chunk);
    });
    child.on("error", (value) => { error ??= value; });
    child.on("close", (status, signal) => {
      clearTimeout(timer);
      resolveResult({ status, signal, error,
        stdout: Buffer.concat(chunks.stdout), stderr: Buffer.concat(chunks.stderr) });
    });
  });
}

export async function exportLdsSource(outputDirectory) {
  const output = resolve(outputDirectory);
  const source = readBounded(join(root, SOURCE_PATH), LIMITS.source_bytes);
  const program = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")),
    "debug/fe2o3-export-sim");
  // Reject special files/symlinks and cap the tool read before executing it.
  readBounded(program, LIMITS.bundle_bytes);
  mkdirSync(output); // Existing evidence is never overwritten.
  const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
  const saveJson = (name, value) => save(name, `${JSON.stringify(value, null, 2)}\n`);
  const env = { ...process.env };
  for (const key of Object.keys(env)) {
    if (key.startsWith("FE2O3_") || key.startsWith("CARGO_TARGET_") && key.endsWith("_RUSTFLAGS")) delete env[key];
  }
  for (const key of ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET"]) delete env[key];
  for (const key of ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"]) env[key] = "";
  env.RUSTC = process.env.RUSTC ?? "rustc";
  const args = ["--crate", "fe2o3_production_ranked_bounds_fixture",
    "--output", join(output, "kernel-v5.fe2sim"), "--target", "gfx942", "--bundle-version", "5",
    "--target-dir", join(output, "extraction"), "--",
    "--manifest-path", join(root, dirname(dirname(SOURCE_PATH)), "Cargo.toml"),
    "--lib", "--features", "workgroup_reduce_u32"];
  try {
    const version = await runBounded(env.RUSTC, ["--version", "--verbose"], env);
    save("rustc-version.stdout", version.stdout);
    save("rustc-version.stderr", version.stderr);
    assert.ifError(version.error);
    assert.equal(version.status, 0, "could not observe selected rustc version");
    assert.ok(version.stdout.length <= 4096 && version.stderr.length <= 4096,
      "rustc version observation exceeded 4096 bytes");
    const versionText = decodeUtf8(version.stdout);
    assert.match(versionText, /^commit-hash: 55e86c996809902e8bbad512cfb4d2c18be446d9$/mu,
      "selected rustc does not match pinned nightly commit");
    assert.match(versionText, /^release: 1\.96\.0-nightly$/mu);
    decodeUtf8(version.stderr);
    saveJson("rustc-version-observation.json", {
      program: env.RUSTC, args: ["--version", "--verbose"], exit_code: version.status,
      stdout_sha256: sha256(version.stdout), stderr_sha256: sha256(version.stderr),
      expected_nightly: "nightly-2026-04-03", pinned_commit_observed: true,
      compiler_closure_attestation: "unavailable",
    });
    const result = await runBounded(program, args, env);
    save("export-stdout.txt", result.stdout);
    save("export-stderr.txt", result.stderr);
    saveJson("export-process-observation.json", {
      program, args, exit_code: result.status, signal: result.signal,
      stdout_sha256: sha256(result.stdout), stderr_sha256: sha256(result.stderr),
      observation_only: true, compiler_closure_attestation: "unavailable",
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, decodeUtf8(result.stderr).slice(0, 4096));
    decodeUtf8(result.stdout);
    assert.match(decodeUtf8(result.stderr), /authority false/u);
    assert.doesNotMatch(decodeUtf8(result.stderr), /authority true/u);
    assert.equal(sha256(readBounded(join(root, SOURCE_PATH), LIMITS.source_bytes)), sha256(source));
    const bundle = readBounded(join(output, "kernel-v5.fe2sim"), LIMITS.bundle_bytes);
    const head = await runBounded("git", ["rev-parse", "HEAD"], env);
    const status = await runBounded("git", ["status", "--porcelain=v1"], env);
    for (const value of [head, status]) { assert.ifError(value.error); assert.equal(value.status, 0); }
    const metadata = {
      schema: "fe2o3-workgroup-u32-v5-export-observation-v1",
      source_path: SOURCE_PATH, source_sha256: sha256(source), bundle_sha256: sha256(bundle),
      command: { program, args, exit_code: 0,
        stdout_sha256: sha256(result.stdout), stderr_sha256: sha256(result.stderr) },
      target: "gfx942:xnack-", nightly: "nightly-2026-04-03",
      checkout_head: decodeUtf8(head.stdout).trim(), checkout_dirty: status.stdout.length !== 0,
      compiler_closure_attestation: "unavailable",
    };
    validateExport(metadata, { source, bundle, stdout: result.stdout, stderr: result.stderr });
    // Only a fully checked invocation produces the input record for the query smoke.
    saveJson("export.json", metadata);
    console.log(`Actual Rust LDS source exported: ${output}`);
    return metadata;
  } catch (error) {
    saveJson("failure.json", { error: String(error), synthetic_fallback: false,
      hardware_observed: false, grants_production_resume: false,
      grants_load_authority: false, grants_launch_authority: false });
    throw error;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 3) throw new Error(
    "usage: node scripts/resource-query-lds-source-export.mjs NEW_SOURCE_EXPORT_DIRECTORY");
  await exportLdsSource(process.argv[2]);
}
