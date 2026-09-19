#!/usr/bin/env node
// CPU tooling diagnostic over the unchanged admitted ordinary-source V6 bundle.
// No compiler invocation, GPU operation, capture fabrication or publication.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { cpus, platform, arch, release } from "node:os";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const PINNED_SOURCE = "f6cdd5fc4a937979fd5d12e7fa199fd3590fae9cb8f303b680781f9ca5b7df5e";
const PINNED_KIR = "9e99fb8946d568b0b8cae097d17925618b0abed23a1f4820ddc1b9e4ab7289f8";
export const LIMITS = Object.freeze({ estimated_records: 8192, estimated_capture_bytes: 64 * 1024 * 1024,
  page_items: 256, page_scanned: 256, commands: 1024, request_bytes: 1024 * 1024,
  response_bytes: 2 * 1024 * 1024, transcript_bytes: 32 * 1024 * 1024,
  per_reply_timeout_ms: 30000, per_case_timeout_ms: 120000, samples: 20, warmup: 3 });

export function scalePrecheck(invocations) {
  assert.ok([4, 64, 128, 256].includes(invocations), "unsupported fixture scale");
  // Exact fixture observed 33 records/invocation. Reserve one extra per invocation
  // plus 64 fixed records. The storage estimate includes a deliberately padded
  // 8192-byte per-record metadata allowance plus a full output-buffer checkpoint.
  // This is an experiment precheck, NOT a simulator capture limit or memory proof.
  const estimatedRecords = 34 * invocations + 64;
  const allocationBytes = 4 * invocations + 8;
  const estimatedCaptureBytes = estimatedRecords * (8192 + allocationBytes);
  const reason = estimatedRecords > LIMITS.estimated_records ? "estimated_records_exceed_8192"
    : estimatedCaptureBytes > LIMITS.estimated_capture_bytes ? "estimated_capture_bytes_exceed_64_MiB" : null;
  return { invocations, allocation_bytes: allocationBytes, estimated_records: estimatedRecords,
    estimated_capture_bytes: estimatedCaptureBytes, status: reason ? "not_run_budget_exceeded" : "admitted_by_script_precheck", reason };
}

export function summarize(values) {
  assert.ok(values.length > 0 && values.length <= 8192);
  assert.ok(Array.from(values).every((value) => Number.isFinite(value) && value >= 0));
  const ordered = [...values].sort((a, b) => a - b);
  return { samples: values.length, min: ordered[0], p50: ordered[Math.ceil(ordered.length * 0.5) - 1],
    p95: ordered[Math.ceil(ordered.length * 0.95) - 1], max: ordered.at(-1), mean: values.reduce((sum, item) => sum + item, 0) / values.length };
}

const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const clone = (value) => JSON.parse(JSON.stringify(value));
const saveJson = (path, value) => writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" });
function processMemory(pid) {
  const status = readFileSync(`/proc/${pid}/status`, "utf8");
  const field = (name) => {
    const match = status.match(new RegExp(`^${name}:\\s+(\\d+) kB$`, "m"));
    assert.ok(match, `missing ${name} process observation`);
    return Number(match[1]) * 1024;
  };
  return { vm_hwm_bytes: field("VmHWM"), vm_rss_bytes: field("VmRSS"),
    interpretation: "Linux whole debugger process resident high-water/current observations; not retained capture-only bytes" };
}

async function runCase({ invocations, output, root, binary, bundlePath, baseRequest }) {
  const precheck = scalePrecheck(invocations);
  assert.equal(precheck.status, "admitted_by_script_precheck");
  const directory = join(output, `invocations-${invocations}`);
  mkdirSync(directory);
  const request = clone(baseRequest);
  request.grid = [invocations, 1, 1];
  request.arguments[0].bytes = `0x${"a5a5a5a5".repeat(invocations)}deadbeefcafebabe`;
  const requestPath = join(directory, "simulation-request.json");
  saveJson(requestPath, request);
  const requests = [], responses = [], timings = [];
  let child, pending, timer, failure, closing = false, buffer = "", stderr = "", totalBytes = 0;
  let revision = 0, id = 0, currentConfig;
  const fail = (error) => { failure ??= error; pending?.reject(error); pending = undefined; child?.kill("SIGKILL"); };
  async function ask(fields) {
    if (failure) throw failure;
    assert.equal(pending, undefined);
    assert.ok(++id <= LIMITS.commands, "case command budget exceeded");
    const request = { schema: "fe2o3-debug-request-v1", request_id: id, expected_revision: revision, ...fields };
    const line = `${JSON.stringify(request)}\n`;
    assert.ok(Buffer.byteLength(line) <= LIMITS.request_bytes);
    requests.push(line);
    const started = performance.now();
    const received = await new Promise((resolveResponse, reject) => {
      const replyTimer = setTimeout(() => fail(new Error("bounded reply deadline exceeded")), LIMITS.per_reply_timeout_ms);
      pending = { resolve(value) { clearTimeout(replyTimer); resolveResponse(value); }, reject(error) { clearTimeout(replyTimer); reject(error); } };
      child.stdin.write(line, (error) => { if (error) fail(error); });
    });
    const elapsed = performance.now() - started;
    const response = received.response;
    assert.equal(response.request_id, id);
    assert.equal(response.operation, request.operation);
    assert.equal(response.schema, request.schema === "fe2o3-debug-resource-request-v1" ? "fe2o3-debug-resource-response-v1" : "fe2o3-debug-response-v1");
    assert.equal(response.status, "ok", JSON.stringify(response.error));
    assert.equal(response.session.backend, "cpu_kir_simulator");
    assert.equal(response.session.simulated, true);
    assert.equal(response.session.hardware_observed, false);
    assert.equal(response.session.performance_prediction, false);
    assert.equal(response.session.execution_kind, "cpu_kir_simulation");
    assert.ok(Number.isSafeInteger(response.session.revision));
    assert.equal(response.session.cursor.state_revision, response.session.revision);
    assert.equal(response.session.cursor.configuration_identity, response.session.configuration_identity);
    currentConfig ??= response.session.configuration_identity;
    assert.equal(response.session.configuration_identity, currentConfig);
    revision = response.session.revision;
    const timing = { request_id: id, roundtrip_ms: elapsed, decode_ms: received.decode_ms, response_bytes: received.bytes };
    timings.push(timing);
    return { response, timing };
  }
  const anchorFrom = (response) => {
    assert.equal(response.result.snapshot.status, "captured");
    const anchor = response.result.snapshot.snapshot.anchor;
    assert.deepEqual(anchor.cursor, response.session.cursor);
    assert.equal(anchor.scope.interpretation, "logical_visualization");
    assert.equal(anchor.site.source.status, "resolved");
    assert.equal(anchor.site.source.location.provenance, "compiler_bundle_bound");
    return clone(anchor);
  };
  async function query(operation, anchor, extra = {}) {
    const result = await ask({ schema: "fe2o3-debug-resource-request-v1", operation, expected_snapshot: anchor,
      page: { max_items: LIMITS.page_items, max_scanned: LIMITS.page_scanned }, ...extra });
    assert.deepEqual(result.response.snapshot, anchor);
    assert.equal(result.response.page.completeness.status, "complete");
    assert.equal(result.response.physical_registers, "not_represented");
    assert.ok(Number.isSafeInteger(result.response.page.source_count));
    assert.ok(result.response.page.scanned <= LIMITS.page_scanned);
    return result;
  }
  async function sweep(anchor, addressSpace) {
    const rows = [], pages = [], tokens = new Set();
    let token, scanned = 0, sourceCount;
    const started = performance.now();
    for (let pageIndex = 0; pageIndex < 32; pageIndex++) {
      const result = await query("query_memory_accesses", anchor, {
        filter: { scope: { level: "dispatch" }, address_space: addressSpace },
        page: { max_items: LIMITS.page_items, max_scanned: LIMITS.page_scanned, ...(token ? { token } : {}) },
      });
      const response = result.response;
      assert.equal(response.result.result, "memory_accesses");
      assert.ok(response.result.accesses.length <= LIMITS.page_items);
      sourceCount ??= response.page.source_count;
      assert.equal(response.page.source_count, sourceCount);
      scanned += response.page.scanned;
      rows.push(...response.result.accesses);
      pages.push({ ...result.timing, rows: response.result.accesses.length, scanned: response.page.scanned });
      token = response.page.next_token;
      if (token === undefined) {
        assert.equal(scanned, sourceCount);
        assert.ok(sourceCount <= precheck.estimated_records && sourceCount <= LIMITS.estimated_records);
        assert.equal(sourceCount, invocations * 33, "fixture profile changed; revisit precheck before increasing budgets");
        assert.ok(rows.length <= invocations);
        for (const [index, row] of rows.entries()) {
          assert.equal(row.occurrence.event_sequence, row.occurrence.record_ordinal + 1);
          assert.ok(row.occurrence.event_sequence <= anchor.cursor.event_sequence);
          assert.equal(row.address_space, "global");
          assert.equal(row.access, "write_committed");
          assert.equal(row.range.byte_len, "4");
          assert.equal(row.range.byte_offset, String(index * 4));
          assert.equal(row.allocation.generation, 0);
          assert.equal(row.source_association, "not_represented");
        }
        assert.equal(rows.length, addressSpace === "global" ? invocations : 0);
        return { sweep_ms: performance.now() - started, source_count: sourceCount, rows: rows.length,
          pages, serialized_response_bytes: pages.reduce((sum, page) => sum + page.response_bytes, 0) };
      }
      assert.ok(typeof token === "string" && token.length <= 128 && !tokens.has(token));
      tokens.add(token);
    }
    throw new Error("page traversal budget exceeded");
  }
  try {
    const started = performance.now();
    child = spawn(binary, ["sim", "--bundle-v6", bundlePath, "--request", requestPath, "--wave-width", "32"], { cwd: root, stdio: ["pipe", "pipe", "pipe"] });
    timer = setTimeout(() => fail(new Error("bounded case deadline exceeded")), LIMITS.per_case_timeout_ms);
    const exited = new Promise((resolveExit) => child.once("exit", (code, signal) => resolveExit({ code, signal })));
    child.once("error", fail);
    child.stdout.setEncoding("utf8"); child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => { stderr += chunk; if (Buffer.byteLength(stderr) > 65536) fail(new Error("stderr budget exceeded")); });
    child.stdout.on("data", (chunk) => {
      try {
        totalBytes += Buffer.byteLength(chunk); buffer += chunk;
        assert.ok(totalBytes <= LIMITS.transcript_bytes && Buffer.byteLength(buffer) <= LIMITS.response_bytes);
        const end = buffer.indexOf("\n");
        if (end < 0) return;
        assert.ok(pending, "unsolicited response");
        const line = buffer.slice(0, end + 1); buffer = buffer.slice(end + 1);
        assert.equal(buffer, "", "one reply per request");
        const decodeStart = performance.now(); const response = JSON.parse(line); const decodeMs = performance.now() - decodeStart;
        responses.push(line);
        const completed = pending; pending = undefined;
        completed.resolve({ response, bytes: Buffer.byteLength(line), decode_ms: decodeMs });
      } catch (error) { fail(error); }
    });
    child.once("exit", (code, signal) => { if (!closing || pending || code !== 0) fail(new Error(`unexpected debugger exit ${code}/${signal}: ${stderr}`)); });
    const first = await ask({ operation: "step", direction: "forward", granularity: "operation", count: 1 });
    anchorFrom(first.response);
    const startupReadyMs = performance.now() - started;
    await ask({ operation: "continue", max_events: LIMITS.estimated_records });
    const checkpoint = await ask({ operation: "step", direction: "reverse", granularity: "operation", count: 1 });
    const anchor = anchorFrom(checkpoint.response);
    const inventory = await query("query_allocations", anchor);
    assert.equal(inventory.response.result.result, "allocations");
    assert.equal(inventory.response.result.allocations.length, 1);
    assert.equal(inventory.response.page.next_token, undefined);
    const allocation = inventory.response.result.allocations[0];
    assert.equal(allocation.capacity_bytes, String(precheck.allocation_bytes));
    assert.equal(allocation.address_space, "global");
    const memory = await ask({ operation: "read_memory", allocation: allocation.allocation, byte_offset: 0, byte_len: precheck.allocation_bytes });
    assert.deepEqual(memory.response.result.snapshot, anchor);
    const available = memory.response.result.memory.availability;
    assert.equal(available.status, "captured");
    // Independent wrapping-u32 oracle over actual fixture scalar arguments.
    const mask = 0xffffffffn, a = BigInt(baseRequest.arguments[1].bits), b = BigInt(baseRequest.arguments[2].bits);
    const oracle = (((((a + b) & mask) - b) & mask) ^ b) & 255n | 256n;
    assert.equal(oracle, 469n);
    const word = Buffer.alloc(4); word.writeUInt32LE(Number(oracle));
    assert.equal(available.bytes, `0x${word.toString("hex").repeat(invocations)}deadbeefcafebabe`);
    assert.equal(available.initialized, `0x${"ff".repeat(Math.ceil(precheck.allocation_bytes / 8))}`);
    assert.equal(available.truncated, false);
    const filters = [];
    for (const addressSpace of ["global", "workgroup"]) {
      const samples = [];
      for (let iteration = 0; iteration < LIMITS.warmup + LIMITS.samples; iteration++) {
        const result = await sweep(anchor, addressSpace);
        if (iteration >= LIMITS.warmup) samples.push(result);
      }
      filters.push({ address_space: addressSpace, actual_matched_rows: samples[0].rows, source_count: samples[0].source_count,
        pages_per_sweep: samples[0].pages.length, scanned_per_sweep: samples[0].pages.reduce((sum, page) => sum + page.scanned, 0),
        sweep_ms: summarize(samples.map((sample) => sample.sweep_ms)),
        page_roundtrip_ms: summarize(samples.flatMap((sample) => sample.pages.map((page) => page.roundtrip_ms))),
        page_decode_ms: summarize(samples.flatMap((sample) => sample.pages.map((page) => page.decode_ms))),
        serialized_response_bytes_per_sweep: summarize(samples.map((sample) => sample.serialized_response_bytes)), samples });
    }
    const memoryObservation = processMemory(child.pid);
    closing = true;
    await ask({ operation: "terminate" }); child.stdin.end();
    assert.deepEqual(await exited, { code: 0, signal: null }); clearTimeout(timer);
    if (failure) throw failure;
    const requestText = requests.join(""), responseText = responses.join("");
    writeFileSync(join(directory, "debug-requests.jsonl"), requestText, { flag: "wx" });
    writeFileSync(join(directory, "debug-responses.jsonl"), responseText, { flag: "wx" });
    writeFileSync(join(directory, "debug-stderr.txt"), stderr, { flag: "wx" });
    saveJson(join(directory, "independent-checkpoint.json"), checkpoint.response);
    saveJson(join(directory, "final-memory.json"), memory.response);
    const result = { ...precheck, status: "passed", configuration_identity: currentConfig, selected_anchor: anchor,
      startup_capture_first_step_ms: startupReadyMs, startup_timing_semantics: "fresh debugger process launch through admission, eager CPU capture, and first step reply; not isolated simulation time",
      actual_records: filters[0].source_count, actual_accesses: invocations, actual_allocation_bytes: precheck.allocation_bytes,
      memory_observation: memoryObservation, whole_process_peak_within_64_MiB: memoryObservation.vm_hwm_bytes <= LIMITS.estimated_capture_bytes,
      commands: id, response_bytes: totalBytes, request_sha256: hash(readFileSync(requestPath)),
      debug_requests_sha256: hash(requestText), debug_responses_sha256: hash(responseText), filters,
      checks: ["independently_anchored_actual_source_checkpoint", "all_output_words_469", "canaries_and_initialization", "exact_complete_bounded_prefix", "all_actual_access_ranges", "empty_filtered_page_continuations"], hardware_observed: false };
    saveJson(join(directory, "receipt.json"), result);
    return { ...result, filters: filters.map(({ samples: _samples, ...rest }) => rest) };
  } catch (error) {
    fail(error); clearTimeout(timer);
    saveJson(join(directory, "failure.json"), { message: String(error), manufactured_capture_fallback: false });
    writeFileSync(join(directory, "partial-requests.jsonl"), requests.join(""), { flag: "wx" });
    writeFileSync(join(directory, "partial-responses.jsonl"), responses.join(""), { flag: "wx" });
    throw error;
  }
}

async function main() {
  if (process.argv.length !== 4) throw new Error("usage: node scripts/resource-query-scale.mjs SOURCE_ASSEMBLY_SMOKE_DIRECTORY NEW_OUTPUT_DIRECTORY");
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const baseline = resolve(process.argv[2]), output = resolve(process.argv[3]);
  mkdirSync(output);
  const receipt = JSON.parse(readFileSync(join(baseline, "receipt.json"), "utf8"));
  assert.equal(receipt.schema, "fe2o3-assembly-authoring-source-smoke-v30");
  assert.equal(receipt.hardware_observed, false);
  const source = "crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30/src/lib.rs";
  assert.equal(receipt.source, source);
  assert.equal(receipt.source_sha256, PINNED_SOURCE, "fixture precheck applies only to pinned source");
  assert.equal(hash(readFileSync(join(root, source))), PINNED_SOURCE);
  assert.equal(receipt.base.summary.canonical_kir_digest, PINNED_KIR);
  const bundlePath = join(baseline, "base-v6.fe2sim"), bundleBytes = readFileSync(bundlePath);
  assert.equal(hash(bundleBytes), receipt.base.bundle_sha256);
  const baseRequest = JSON.parse(readFileSync(join(baseline, "base-request.json"), "utf8"));
  assert.equal(baseRequest.schema, "fe2o3-simulation-request-v1");
  assert.equal(baseRequest.kernel, "assembly_chain");
  assert.deepEqual(baseRequest.grid, [4, 1, 1]); assert.deepEqual(baseRequest.workgroup, [64, 1, 1]);
  assert.equal(baseRequest.arguments.length, 3);
  assert.deepEqual(baseRequest.arguments[0], { kind: "buffer", element: "u32", access: "read_write", alignment: 4,
    bytes: "0xa5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5deadbeefcafebabe" });
  assert.deepEqual(baseRequest.arguments.slice(1), [{ kind: "scalar", type: "u32", bits: "0xfffffff0" }, { kind: "scalar", type: "u32", bits: "0x00000025" }]);
  const binary = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug", "fe2o3-debug");
  const binaryHash = hash(readFileSync(binary));
  const cases = [];
  for (const invocations of [4, 64, 128, 256]) {
    const check = scalePrecheck(invocations);
    cases.push(check.status === "not_run_budget_exceeded" ? check : await runCase({ invocations, output, root, binary, bundlePath, baseRequest }));
  }
  assert.equal(hash(readFileSync(join(root, source))), PINNED_SOURCE);
  assert.equal(hash(readFileSync(bundlePath)), receipt.base.bundle_sha256);
  assert.equal(hash(readFileSync(binary)), binaryHash, "debugger binary changed during measurements");
  const report = { schema: "fe2o3-resource-query-scale-diagnostic-v1", classification: "CPU_debugger_tooling_not_GPU_performance", source, source_sha256: PINNED_SOURCE,
    measured_at: new Date().toISOString(), measurement_script_sha256: hash(readFileSync(fileURLToPath(import.meta.url))),
    bundle_sha256: receipt.base.bundle_sha256, canonical_kir_digest: PINNED_KIR, debugger_binary_sha256: binaryHash,
    environment: { node: process.version, platform: platform(), arch: arch(), os_release: release(), cpu_model: cpus()[0]?.model ?? "unavailable", logical_cpus: cpus().length },
    script_precheck_limits: LIMITS, precheck_semantics: "fixture-specific padded storage estimate; not a runtime capture budget or guaranteed allocation limit",
    existing_CLI_hard_capture_limits: { records: 1000000, resident_total_bytes: 256 * 1024 * 1024, overridable_by_this_script: false },
    memory_semantics: "script estimates recorded separately from measured whole-child-process Linux VmHWM/RSS",
    query_timing_semantics: "sequential local JSONL request/write to parsed response, including backend query, transport and JSON decode; sweep also includes assertions; no GPU or physical-register inference",
    diagnostic_budget_ms: { page_roundtrip_p95: 25, full_sweep_p95: 250, advisory_not_CI_gate: true },
    diagnostic_budget_results: cases.filter((item) => item.status === "passed").map((item) => ({ invocations: item.invocations,
      filters: item.filters.map((filter) => ({ address_space: filter.address_space,
        page_roundtrip_within_advisory_budget: filter.page_roundtrip_ms.p95 <= 25, full_sweep_within_advisory_budget: filter.sweep_ms.p95 <= 250 })) })),
    cases, hardware_observed: false, source_edited: false, source_recompiled: false, original_capture_modified: false, curriculum_publication: "not_performed" };
  saveJson(join(output, "receipt.json"), report);
  console.log(JSON.stringify(report, null, 2));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
