#!/usr/bin/env node
// Public Cargo/source workflow. No private rustc callbacks, synthetic owners,
// native compiler worker, artifact finalizer, runtime, or GPU are invoked.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, constants, existsSync, fstatSync, lstatSync, mkdirSync, openSync, readdirSync, readSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseExactJson } from "./ordered-region-debugger-client.mjs";

const cases = {
  one: ["complete-body-one-v19", null],
  diamond: ["complete-body-diamond-v19", null],
};
if (process.argv.length !== 4 || !Object.hasOwn(cases, process.argv[2])) {
  throw new Error("usage: node scripts/complete-body-debug-source-v19.mjs one|diamond NEW_OUTPUT_DIRECTORY");
}
assert.equal(process.platform, "linux", "this bounded Cargo workflow requires Linux");
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = resolve(process.argv[3]);
const [feature] = cases[process.argv[2]];
const fixture = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
const rustc = process.env.RUSTC;
assert.ok(rustc && isAbsolute(rustc), "set RUSTC to the absolute pinned-nightly binary (no auto-install)");
const bin = resolve(process.env.FE2O3_COMPLETE_BODY_BIN_DIR_V19 ??
  join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug"));
const wrapper = join(bin, "fe2o3-rustc-extract");
const simulator = join(bin, "fe2o3-kir-sim");
const debuggerBinary = join(bin, "fe2o3-debug");
mkdirSync(output); // Must be new. Never overwrite or reuse an earlier run.
const MiB = 1024 * 1024;
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx", mode: 0o600 });
const saveJson = (name, value) => save(name, JSON.stringify(value, null, 2) + "\n");
const observations = [];
const sources = [
  "Cargo.toml", "src/lib.rs", "src/complete_body_v19.rs",
].map(relative => join(root, fixture, relative)).concat([
  fileURLToPath(import.meta.url), join(root, "scripts/ordered-region-debugger-client.mjs"),
]);

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
async function run(command, args, env, input = undefined) {
  return new Promise(resolveResult => {
    const child = spawn(command, args, { cwd: root, env, detached: true, stdio: [input === undefined ? "ignore" : "pipe", "pipe", "pipe"] });
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
    if (input !== undefined) {
      assert.ok(input.length <= MiB, "bounded debugger input");
      child.stdin.on("error", value => { error ??= value; });
      child.stdin.end(input);
    }
    child.on("close", (status, signal) => {
      clearTimeout(timeout); clearInterval(disk);
      resolveResult({ status, signal, error,
        stdout: Buffer.concat(chunks.stdout), stderr: Buffer.concat(chunks.stderr) });
    });
  });
}

const decode = bytes => new TextDecoder("utf-8", { fatal: true }).decode(bytes);
const binaryPaths = [wrapper, join(bin, "librustc_codegen_fe2o3.so"), simulator, debuggerBinary];
async function checkedRun(stage, command, args, env, input) {
  const result = await run(command, args, env, input);
  save(stage + ".stdout", result.stdout); save(stage + ".stderr", result.stderr);
  const observation = { stage, command, args, status: result.status, signal: result.signal,
    stdout_sha256: hash(result.stdout), stderr_sha256: hash(result.stderr) };
  observations.push(observation);
  saveJson(stage + "-process.json", observation);
  assert.ifError(result.error);
  assert.equal(result.signal, null, "a signal is not a successful observation");
  assert.equal(result.status, 0, decode(result.stderr));
  decode(result.stdout); decode(result.stderr);
  observeFreshDisk();
  return result;
}
try {
  const sourcePins = sources.map(path => pin(path));
  const binaryPins = binaryPaths.map(path => pin(path, 512 * MiB));
  const sysrootResult = await checkedRun("sysroot", rustc, ["--print", "sysroot"], sanitized());
  const sysroot = decode(sysrootResult.stdout).trimEnd();
  assert.ok(isAbsolute(sysroot) && sysroot.length <= 4096 && !/[\r\n]/u.test(sysroot));
  assert.ok(basename(sysroot).startsWith("nightly-2026-04-03-"), "wrong pinned compiler");
  saveJson("inputs.json", { schema: "fe2o3-complete-body-debug-inputs-v19",
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
  const kirPath = join(output, "diagnostic-v19.kir");
  const args = ["check", "--release", "--locked", "--offline", "-Zbuild-std=core", "--lib",
    "--manifest-path", join(root, fixture, "Cargo.toml"), "--no-default-features", "--features", feature,
    "--target", "amdgcn-amd-amdhsa", "--target-dir", join(output, "source-target")];
  const extracted = await checkedRun("source", join(sysroot, "bin/cargo"), args,
    { ...env, FE2O3_EXTRACT_DIAGNOSTIC_KIR_PATH_V19: kirPath });
  const marker = "fe2o3 diagnostic KIR19 input: ";
  const lines = decode(extracted.stderr).split("\n").filter(line => line.startsWith(marker));
  assert.equal(lines.length, 1, "expected one actual-source exporter observation");
  const metadata = JSON.parse(lines[0].slice(marker.length));
  assert.equal(metadata.schema, "fe2o3-diagnostic-kir-v19-observation");
  assert.ok(typeof metadata.kernel === "string" && Buffer.byteLength(metadata.kernel) <= 4096);
  for (const key of ["exported_source_authentication", "exported_compiler_authentication",
    "artifact_or_launch_authority", "hardware_observed"]) assert.equal(metadata[key], false);
  for (const key of ["canonical_identity", "semantic_mir_v36"]) {
    assert.match(metadata[key], /^[0-9a-f]{64}$/);
  }
  const kir = readBounded(kirPath, 16 * MiB);
  assert.ok(kir.length > 10);
  assert.equal(kir.readUInt16LE(8), 19, "exact explicit wire version");
  assert.equal(metadata.canonical_bytes, kir.length);
  saveJson("export-observation.json", metadata);
  const kirPin = pin(kirPath, 16 * MiB);
  const diagnosticEnv = { ...sanitized(), LD_LIBRARY_PATH: env.LD_LIBRARY_PATH };
  const requests = [];
  for (const selector of [0, 1, 0xffffffff]) {
    const prefix = "selector-" + selector;
    // The public demonstration allocates >=512B (the conservative source
    // LaunchEnvelope requirement for max_grid2) and adds two untouched words.
    const initial = Buffer.alloc(130 * 4, 0x5a);
    const scalar = value => ({ kind: "scalar", type: "u32",
      bits: "0x" + value.toString(16).padStart(8, "0") });
    const request = { schema: "fe2o3-simulation-request-v1", kernel: metadata.kernel,
      grid: [128, 1, 1], workgroup: [64, 1, 1],
      arguments: [{ kind: "buffer", element: "u32", access: "read_write",
        alignment: 4, bytes: "0x" + initial.toString("hex") },
        scalar(19), scalar(23), scalar(42), scalar(selector)] };
    const requestName = prefix + "-request.json";
    saveJson(requestName, request);
    const requestPath = join(output, requestName), resultPath = join(output, prefix + "-cpu.json");
    await checkedRun(prefix + "-cpu", simulator,
      ["--diagnostic-kir-v19", kirPath, "--request", requestPath, "--output", resultPath], diagnosticEnv);
    const result = parseExactJson(decode(readBounded(resultPath, MiB)));
    assert.equal(result.schema, "fe2o3-simulation-result-v1");
    assert.equal(result.status, "ok");
    assert.equal(result.authority, "observation_only");
    assert.equal(result.simulated, true);
    for (const key of ["hardware_observed", "hardware_validation", "performance_prediction"]) {
      assert.equal(result[key], false);
    }
    assert.equal(result.kir.sha256, metadata.canonical_identity);
    assert.equal(result.kir.canonical_bytes, kir.length);
    assert.equal(result.counts.invocations_executed, 128);
    const expectedValue = feature === "complete-body-diamond-v19" && selector !== 0 ? 23 : 19;
    const expected = Buffer.from(initial);
    for (let i = 0; i < 128; i++) expected.writeUInt32LE(expectedValue, i * 4);
    assert.equal(result.arguments[0].value.bytes, "0x" + expected.toString("hex"),
      "written values or trailing canaries differ");
    assert.equal(result.arguments[0].value.initialized, "0x" + "ff".repeat(65));

    // Entry -> first logical event -> entry. These are replay observations,
    // not a physical VGPR/EXEC capture or an instruction timing measurement.
    const operations = [
      { operation: "discover_capabilities", expected_revision: 0 },
      { operation: "get_state", expected_revision: 0 },
      { operation: "step", expected_revision: 0, direction: "forward", granularity: "event", count: 1 },
      { operation: "step", expected_revision: 1, direction: "reverse", granularity: "event", count: 1 },
      { operation: "get_state", expected_revision: 2 },
    ];
    const protocol = operations.map((operation, index) => JSON.stringify({
      schema: "fe2o3-debug-request-v1", request_id: index + 1, ...operation,
    })).join("\n") + "\n";
    save(prefix + "-debug-requests.jsonl", protocol);
    const debug = await checkedRun(prefix + "-debug", debuggerBinary,
      ["sim", "--diagnostic-kir-v19", kirPath, "--request", requestPath,
        "--protocol", "jsonl", "--wave-width", "64"], diagnosticEnv, Buffer.from(protocol));
    const responses = decode(debug.stdout).trimEnd().split("\n").map(parseExactJson);
    assert.equal(responses.length, operations.length);
    let configuration;
    responses.forEach((response, index) => {
      assert.equal(response.schema, "fe2o3-debug-response-v1");
      assert.equal(response.request_id, index + 1);
      assert.equal(response.operation, operations[index].operation);
      assert.equal(response.status, "ok");
      assert.equal(response.session.backend, "cpu_kir_simulator");
      assert.equal(response.session.execution_kind, "cpu_kir_simulation");
      assert.equal(response.session.simulated, true);
      assert.equal(response.session.hardware_observed, false);
      assert.equal(response.session.performance_prediction, false);
      assert.equal(response.session.revision, [0, 0, 1, 2, 2][index]);
      configuration ??= response.session.configuration_identity;
      assert.equal(response.session.configuration_identity, configuration);
    });
    for (const [name, reason] of [["register_values", "not_represented"],
      ["source_sites", "requires_authenticated_map"], ["hardware_wave_state", "logical_visualization_only"]]) {
      assert.deepEqual(responses[0].result.capabilities.find(row => row.name === name),
        { name, availability: "unavailable", reason });
    }
    assert.equal(responses[4].session.cursor.event_sequence, responses[1].session.cursor.event_sequence);
    assert.ok(responses[2].session.cursor.event_sequence > responses[1].session.cursor.event_sequence);
    requests.push({ selector, expectedValue, request: pin(requestPath),
      result: pin(resultPath), initialized_output_and_canaries_match: true,
      logical_forward_reverse_observed: true, configuration });
    assert.deepEqual(pin(kirPath, 16 * MiB), kirPin);
    assert.deepEqual(sources.map(path => pin(path)), sourcePins);
    assert.deepEqual(binaryPaths.map(path => pin(path, 512 * MiB)), binaryPins);
    observeFreshDisk();
  }
  saveJson("receipt.json", { schema: "fe2o3-complete-body-source-debug-command-v19",
    feature, metadata, kir: kirPin, sourcePins, binaryPins, requests, observations,
    actual_cargo_rustc_wrapper: true, source_unchanged: true,
    raw_canonical_bytes_preserved: true, cpu_simulation_run: true,
    logical_debugger_forward_reverse: true, compiler_closure_attestation: "unavailable",
    exported_source_authentication: false, exported_compiler_authentication: false,
    physical_register_values: "unavailable", source_variable_map: "unavailable",
    persisted_schedule: "unavailable", runtime_observations: "unavailable",
    diagnosis_v2: "unavailable", protected_finalizer_admitted: false,
    native_llvm_executed: false, hardware_observed: false,
    grants_artifact_or_launch_authority: false });
  console.log("Actual-source KIR19 -> CPU and logical debugger observations retained: " + output);
} catch (error) {
  saveJson("failure.json", { schema: "fe2o3-complete-body-source-debug-command-failure-v19",
    error: String(error), feature, observations, synthetic_fallback: false,
    native_llvm_executed: false, hardware_observed: false });
  throw error;
}
