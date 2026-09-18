#!/usr/bin/env node
// Query an actually exported source Bundle V6. No builds, source edits, invented
// captures, or fallback IR. Input receipts link files; debugger admission remains
// responsible for validating the bundle and its compiler-bound source map.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.argv.length !== 4) {
  throw new Error("usage: node scripts/resource-query-v6-smoke.mjs SOURCE_ASSEMBLY_SMOKE_DIRECTORY NEW_OUTPUT_DIRECTORY");
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const baseline = resolve(process.argv[2]);
const output = resolve(process.argv[3]);
mkdirSync(output); // Deliberately fails for an existing evidence directory.
const bin = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug");
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
const saveJson = (name, value) => save(name, `${JSON.stringify(value, null, 2)}\n`);
const clone = (value) => JSON.parse(JSON.stringify(value));
const requests = [], responses = [], requestLines = [], responseLines = [];
let child, pending, failure, processTimer, closing = false, stderr = "", stdout = "", responseBytes = 0;
let revision = 0, nextId = 1;

function rejectRun(error) {
  failure ??= error;
  clearTimeout(processTimer);
  pending?.reject(error);
  pending = undefined;
  child?.kill("SIGKILL");
}

function checkSession(response) {
  assert.equal(response.session.backend, "cpu_kir_simulator");
  assert.equal(response.session.execution_kind, "cpu_kir_simulation");
  assert.equal(response.session.simulated, true);
  assert.equal(response.session.hardware_observed, false);
  assert.equal(response.session.performance_prediction, false);
  assert.equal(response.session.cursor.state_revision, response.session.revision);
  assert.equal(response.session.cursor.configuration_identity, response.session.configuration_identity);
  assert.ok(Number.isSafeInteger(response.session.revision));
}

async function ask(command) {
  if (failure) throw failure;
  assert.equal(pending, undefined, "only one query may be pending");
  assert.ok(nextId <= 1024, "bounded command budget");
  const request = { schema: "fe2o3-debug-request-v1", request_id: nextId++, expected_revision: revision, ...command };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert.ok(Buffer.byteLength(requestLine) <= 1024 * 1024, "request line byte budget");
  requests.push(request);
  requestLines.push(requestLine);
  const response = await new Promise((resolveResponse, reject) => {
    const timer = setTimeout(() => rejectRun(new Error("debugger response timeout")), 30000);
    pending = {
      resolve(value) { clearTimeout(timer); resolveResponse(value); },
      reject(error) { clearTimeout(timer); reject(error); },
    };
    child.stdin.write(requestLine, (error) => { if (error) rejectRun(error); });
  });
  assert.equal(response.request_id, request.request_id);
  assert.equal(response.operation, request.operation);
  assert.equal(response.schema, request.schema === "fe2o3-debug-resource-request-v1"
    ? "fe2o3-debug-resource-response-v1" : "fe2o3-debug-response-v1");
  checkSession(response);
  revision = response.session.revision;
  return response;
}

function snapshot(response, sourceRequired = false) {
  assert.equal(response.status, "ok");
  const captured = response.result.snapshot;
  assert.equal(captured.status, "captured");
  const anchor = captured.snapshot.anchor;
  assert.deepEqual(anchor.cursor, response.session.cursor);
  assert.equal(anchor.scope.level, "lane");
  assert.equal(anchor.scope.interpretation, "logical_visualization");
  if (sourceRequired) {
    assert.equal(anchor.site.source.status, "resolved");
    assert.equal(anchor.site.source.location.provenance, "compiler_bundle_bound");
  }
  return clone(anchor);
}

async function step(direction, sourceRequired = false) {
  const response = await ask({ operation: "step", direction, granularity: "operation", count: 1 });
  return { response, anchor: snapshot(response, sourceRequired) };
}

async function resource(operation, anchor, fields = {}) {
  const response = await ask({ schema: "fe2o3-debug-resource-request-v1", operation, expected_snapshot: anchor,
    page: { max_items: 16, max_scanned: 16 }, ...fields });
  assert.equal(response.status, "ok");
  assert.deepEqual(response.snapshot, anchor);
  assert.equal(response.physical_registers, "not_represented");
  assert.equal(response.page.completeness.status, "complete");
  assert.ok(response.page.scanned <= (fields.page?.max_scanned ?? 16));
  assert.ok(Number.isSafeInteger(response.page.source_count));
  return response;
}

async function accessPages(anchor) {
  const rows = [], seen = new Set();
  let token, scanned = 0, sourceCount;
  for (let pageIndex = 0; pageIndex < 256; pageIndex++) {
    const response = await resource("query_memory_accesses", anchor, {
      filter: { scope: { level: "dispatch" } }, page: { max_items: 4, max_scanned: 16, ...(token ? { token } : {}) },
    });
    assert.equal(response.result.result, "memory_accesses");
    sourceCount ??= response.page.source_count;
    assert.equal(response.page.source_count, sourceCount);
    scanned += response.page.scanned;
    for (const row of response.result.accesses) {
      assert.ok(!seen.has(row.occurrence.event_sequence), "no duplicate access occurrence");
      assert.equal(row.occurrence.event_sequence, row.occurrence.record_ordinal + 1);
      assert.ok(row.occurrence.event_sequence <= anchor.cursor.event_sequence);
      assert.equal(row.source_association, "not_represented");
      assert.equal(row.call_frame, "not_represented");
      assert.equal(row.operation_occurrence, "not_represented");
      assert.equal(typeof row.range.byte_offset, "string");
      assert.equal(typeof row.range.byte_len, "string");
      seen.add(row.occurrence.event_sequence);
      rows.push(row);
    }
    const next = response.page.next_token;
    if (next === undefined) {
      assert.equal(scanned, sourceCount);
      return rows;
    }
    assert.notEqual(next, token);
    token = next;
  }
  throw new Error("resource access page budget exhausted");
}

async function memory(anchor, allocation, expected) {
  const response = await ask({ operation: "read_memory", allocation, byte_offset: 0, byte_len: 24 });
  assert.equal(response.status, "ok");
  assert.deepEqual(response.result.snapshot, anchor);
  assert.deepEqual(response.result.memory.allocation, allocation);
  assert.equal(response.result.memory.availability.status, "captured");
  assert.equal(response.result.memory.availability.bytes, expected);
  assert.equal(response.result.memory.availability.initialized, "0xffffff");
  assert.equal(response.result.memory.availability.truncated, false);
  return response;
}

async function rejected(command, expectedCode) {
  const before = revision;
  const response = await ask(command);
  assert.equal(response.status, "error");
  assert.equal(response.error.code, expectedCode);
  assert.equal(response.error.state_changed, false);
  assert.equal(revision, before);
  return response;
}

try {
  const receipt = JSON.parse(readFileSync(join(baseline, "receipt.json"), "utf8"));
  assert.equal(receipt.schema, "fe2o3-assembly-authoring-source-smoke-v30");
  assert.equal(receipt.hardware_observed, false);
  const sourcePath = "crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30/src/lib.rs";
  assert.equal(receipt.source, sourcePath);
  const sourceBytes = readFileSync(join(root, sourcePath));
  assert.equal(hash(sourceBytes), receipt.source_sha256);
  const bundlePath = join(baseline, "base-v6.fe2sim");
  const bundle = readFileSync(bundlePath);
  assert.equal(hash(bundle), receipt.base.bundle_sha256);
  const requestPath = join(baseline, "base-request.json");
  const simulationRequestBytes = readFileSync(requestPath);
  const simulationRequest = JSON.parse(simulationRequestBytes);
  assert.equal(simulationRequest.kernel, "assembly_chain");
  assert.deepEqual(simulationRequest.grid, [4, 1, 1]);
  assert.deepEqual(simulationRequest.workgroup, [64, 1, 1]);
  assert.equal(simulationRequest.arguments[1].bits, "0xfffffff0");
  assert.equal(simulationRequest.arguments[2].bits, "0x00000025");
  const canary = "deadbeefcafebabe";
  const beforeBytes = `0x${"a5a5a5a5".repeat(4)}${canary}`;
  assert.equal(simulationRequest.arguments[0].bytes, beforeBytes);
  const mask = 0xffffffffn, a = 0xfffffff0n, b = 0x25n;
  const expectedWord = (((((a + b) & mask) - b) & mask) ^ b) & 255n | 256n;
  assert.equal(expectedWord, 469n);
  const word = Buffer.alloc(4); word.writeUInt32LE(Number(expectedWord));
  const afterFirstBytes = `0x${word.toString("hex")}${"a5a5a5a5".repeat(3)}${canary}`;
  const finalBytes = `0x${word.toString("hex").repeat(4)}${canary}`;
  const inspected = spawnSync(join(bin, "fe2o3-author"), ["inspect"], { cwd: root, input: bundle, encoding: "utf8", maxBuffer: 2 * 1024 * 1024, timeout: 30000 });
  assert.equal(inspected.error, undefined);
  assert.equal(inspected.status, 0, inspected.stderr);
  const summary = JSON.parse(inspected.stdout);
  assert.equal(summary.bundle_identity, receipt.base.summary.bundle_identity);
  assert.equal(summary.canonical_kir_digest, receipt.base.summary.canonical_kir_digest);
  assert.equal(summary.canonical_kir_version, 11);
  assert.equal(summary.authority.grants_production_resume, false);
  save("admitted-inspection.json", inspected.stdout);

  child = spawn(join(bin, "fe2o3-debug"), ["sim", "--bundle-v6", bundlePath, "--request", requestPath, "--wave-width", "32"], { cwd: root, stdio: ["pipe", "pipe", "pipe"] });
  processTimer = setTimeout(() => rejectRun(new Error("debugger process timeout")), 120000);
  const exited = new Promise((resolveExit) => child.once("exit", (code, signal) => resolveExit({ code, signal })));
  child.once("error", rejectRun);
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
    if (Buffer.byteLength(stderr) > 65536) rejectRun(new Error("debugger stderr bound exceeded"));
  });
  child.stdout.on("data", (chunk) => {
    try {
      responseBytes += Buffer.byteLength(chunk);
      stdout += chunk;
      assert.ok(responseBytes <= 16 * 1024 * 1024, "transcript byte budget");
      assert.ok(Buffer.byteLength(stdout) <= 2 * 1024 * 1024, "response line byte budget");
      const newline = stdout.indexOf("\n");
      if (newline < 0) return;
      assert.ok(pending, "no unsolicited debugger response");
      const responseLine = stdout.slice(0, newline + 1);
      const response = JSON.parse(responseLine);
      stdout = stdout.slice(newline + 1);
      assert.equal(stdout, "", "one request must yield exactly one response");
      responses.push(response);
      responseLines.push(responseLine);
      const completed = pending; pending = undefined;
      completed.resolve(response);
    } catch (error) { rejectRun(error); }
  });
  child.once("exit", (code, signal) => {
    if (!closing || pending || code !== 0) rejectRun(new Error(`debugger exited unexpectedly: ${code}/${signal}: ${stderr}`));
  });

  const initial = await step("forward");
  const initialInventory = await resource("query_allocations", initial.anchor);
  assert.equal(initialInventory.result.result, "allocations");
  assert.equal(initialInventory.result.allocations.length, 1);
  const allocationRow = initialInventory.result.allocations[0];
  assert.equal(allocationRow.capacity_bytes, "24");
  assert.equal(allocationRow.address_space, "global");
  assert.equal(allocationRow.owning_scope, "not_represented");
  assert.equal(allocationRow.lifetime, "not_represented");
  assert.equal(allocationRow.physical_base, "not_represented");
  const allocation = allocationRow.allocation;
  assert.equal(allocation.generation, 0);
  await memory(initial.anchor, allocation, beforeBytes);
  assert.equal((await ask({ operation: "set_watchpoints", watchpoints: [{ client_label: "source-first-write", enabled: true, allocation, byte_offset: 0, byte_len: 4, access: "write", timing: "after_commit" }] })).status, "ok");
  assert.equal((await ask({ operation: "continue", max_events: 65536 })).status, "ok");
  const post = await step("forward", true);
  const inventory = await resource("query_allocations", post.anchor);
  const firstAccesses = await accessPages(post.anchor);
  assert.equal(firstAccesses.length, 1);
  assert.equal(firstAccesses[0].range.byte_offset, "0");
  assert.equal(firstAccesses[0].range.byte_len, "4");
  assert.equal(firstAccesses[0].access, "write_committed");
  const postMemory = await memory(post.anchor, allocation, afterFirstBytes);
  const staleTokenPage = await resource("query_memory_accesses", post.anchor, { filter: { scope: { level: "dispatch" } }, page: { max_items: 1, max_scanned: 1 } });
  assert.equal(typeof staleTokenPage.page.next_token, "string");
  const wrongSource = clone(post.anchor);
  wrongSource.site.source = { status: "unavailable", reason: "not_represented" };
  await rejected({ schema: "fe2o3-debug-resource-request-v1", operation: "query_allocations", expected_snapshot: wrongSource, page: { max_items: 1, max_scanned: 1 } }, "invalid_cursor");
  const reverse = await step("reverse", true);
  const reverseMemory = await memory(reverse.anchor, allocation, beforeBytes);
  assert.equal((await accessPages(reverse.anchor)).length, 0, "future committed access must be excluded");
  const forwardAgain = await step("forward", true);
  assert.equal(forwardAgain.anchor.cursor.event_sequence, post.anchor.cursor.event_sequence);
  assert.notEqual(forwardAgain.anchor.cursor.state_revision, post.anchor.cursor.state_revision);
  await rejected({ schema: "fe2o3-debug-resource-request-v1", operation: "query_allocations", expected_revision: post.anchor.cursor.state_revision, expected_snapshot: post.anchor, page: { max_items: 1, max_scanned: 1 } }, "stale_revision");
  await rejected({ schema: "fe2o3-debug-resource-request-v1", operation: "query_memory_accesses", expected_snapshot: forwardAgain.anchor, filter: { scope: { level: "dispatch" } }, page: { max_items: 1, max_scanned: 1, token: staleTokenPage.page.next_token } }, "invalid_cursor");
  await memory(forwardAgain.anchor, allocation, afterFirstBytes);
  assert.equal((await ask({ operation: "continue", max_events: 65536 })).status, "ok");
  const final = await step("reverse");
  const finalMemory = await memory(final.anchor, allocation, finalBytes);
  const finalAccesses = await accessPages(final.anchor);
  assert.deepEqual(finalAccesses.map((row) => row.range.byte_offset), ["0", "4", "8", "12"]);
  closing = true;
  assert.equal((await ask({ operation: "terminate" })).status, "ok");
  child.stdin.end();
  assert.deepEqual(await exited, { code: 0, signal: null });
  clearTimeout(processTimer);
  if (failure) throw failure;
  assert.equal(hash(readFileSync(join(root, sourcePath))), receipt.source_sha256);
  const requestText = requestLines.join("");
  const responseText = responseLines.join("");
  save("debug-requests.jsonl", requestText);
  save("debug-responses.jsonl", responseText);
  save("debug-stderr.txt", stderr);
  saveJson("resource-checkpoint.json", { schema: "fe2o3-resource-checkpoint-example-v1", origin: "ordinary_rust_source_export", source: sourcePath, bundle_sha256: hash(bundle), expectedSnapshot: post.anchor, response: postMemory });
  saveJson("resource-query-results.json", { expected_snapshot: post.anchor, independent_anchor_response: post.response, inventory, accesses: firstAccesses, reverse_anchor_response: reverse.response, reverse_memory: reverseMemory, final_memory: finalMemory, final_accesses: finalAccesses });
  saveJson("receipt.json", { schema: "fe2o3-resource-query-source-smoke-v1", source: sourcePath, source_sha256: receipt.source_sha256, bundle_sha256: hash(bundle), bundle_identity: summary.bundle_identity, canonical_kir_digest: summary.canonical_kir_digest, simulation_request_sha256: hash(simulationRequestBytes), debug_requests_sha256: hash(requestText), debug_responses_sha256: hash(responseText), expected_u32: Number(expectedWord), checks: ["actual_admitted_source_bundle", "independent_full_checkpoint_anchor", "bounded_allocation_inventory", "bounded_actual_memory_access_pages", "first_word_and_four_word_independent_oracle", "initialization_and_canaries", "reverse_prewrite_bytes", "no_future_accesses", "same_cursor_new_revision", "stale_full_anchor_and_token_rejection"], source_edited: false, hardware_observed: false, physical_registers: "not_represented", allocation_lifetime: "not_represented", curriculum_publication: "not_performed" });
  console.log(`Actual-source V6 resource queries passed; evidence: ${output}`);
} catch (error) {
  rejectRun(error);
  saveJson("failure.json", { status: "failed", message: String(error), manufactured_capture_fallback: false, hardware_observed: false });
  saveJson("partial-transcript.json", { requests, responses });
  save("failure-stderr.txt", stderr);
  throw error;
}
