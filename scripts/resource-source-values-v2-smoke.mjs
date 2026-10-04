#!/usr/bin/env node
// Qualification of existing public CPU debugger queries; no new wire schema.
// Importing this file exposes only pure checks and starts no child or file work.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { lstatSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const V1 = "fe2o3-debug-request-v1", V2 = "fe2o3-debug-source-variable-request-v2";
const expectedSchema = schema => schema === V2 ? "fe2o3-debug-source-variable-response-v2"
  : schema === "fe2o3-debug-resource-request-v1" ? "fe2o3-debug-resource-response-v1" : "fe2o3-debug-response-v1";
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const copy = value => structuredClone(value);
function integer(value, minimum = 0, maximum = Number.MAX_SAFE_INTEGER) {
  assert.ok(Number.isSafeInteger(value) && value >= minimum && value <= maximum, "bounded exact integer");
  return value;
}
function exact(value, keys) {
  assert.ok(value && typeof value === "object" && !Array.isArray(value), "object required");
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), "closed observation shape");
  return value;
}
function digest(value) {
  assert.equal(typeof value, "string"); assert.match(value, /^[0-9a-f]{64}$/u);
  assert.notEqual(value, "0".repeat(64)); return value;
}
function session(value) {
  exact(value, ["backend", "execution_kind", "state", "revision", "configuration_identity", "cursor",
    "simulated", "hardware_observed", "performance_prediction"]);
  assert.equal(value.backend, "cpu_kir_simulator"); assert.equal(value.execution_kind, "cpu_kir_simulation");
  assert.equal(value.state, "stopped"); assert.equal(value.simulated, true);
  assert.equal(value.hardware_observed, false); assert.equal(value.performance_prediction, false);
  integer(value.revision); digest(value.configuration_identity);
  assert.deepEqual(value.cursor, { configuration_identity: value.configuration_identity,
    event_sequence: integer(value.cursor.event_sequence), state_revision: value.revision });
  return value;
}
function paired(pair, operation, schema = V1) {
  assert.equal(pair.request.schema, schema); assert.equal(pair.response.schema, expectedSchema(schema));
  integer(pair.request.request_id, 1); integer(pair.request.expected_revision);
  assert.equal(pair.response.request_id, pair.request.request_id);
  assert.equal(pair.request.operation, operation); assert.equal(pair.response.operation, operation);
  assert.equal(pair.response.status, "ok"); session(pair.response.session);
}
function unchanged(pair, operation, stopped, schema = V1) {
  paired(pair, operation, schema);
  assert.equal(pair.request.expected_revision, stopped.revision);
  assert.deepEqual(pair.response.session, stopped, "read-only query must remain at this exact stop");
}
function checkpoint(pair) {
  paired(pair, "step");
  exact(pair.request, ["schema", "request_id", "expected_revision", "operation", "direction", "granularity", "count"]);
  assert.ok(["forward", "reverse"].includes(pair.request.direction));
  assert.equal(pair.request.granularity, "operation"); assert.equal(pair.request.count, 2);
  assert.equal(pair.response.session.revision, pair.request.expected_revision + 1);
  const result = exact(pair.response.result, ["result", "stop", "snapshot", "events_advanced"]);
  assert.equal(result.result, "control"); integer(result.events_advanced, 1, 65536);
  assert.deepEqual(result.stop, { reason: "step", outcome: "active", exact: true });
  exact(result.snapshot, ["status", "snapshot"]); assert.equal(result.snapshot.status, "captured");
  const snapshot = exact(result.snapshot.snapshot, ["anchor", "stop", "values"]);
  assert.deepEqual(snapshot.stop, result.stop);
  const anchor = exact(snapshot.anchor, ["cursor", "scope", "site"]);
  assert.deepEqual(anchor.cursor, pair.response.session.cursor);
  assert.deepEqual(anchor.scope, { level: "lane", workgroup: [0, 0, 0], wave: 0, lane: 0,
    logical_workitem: [0, 0, 0], active_mask: 15, wave_width: 32, interpretation: "logical_visualization" });
  const site = exact(anchor.site, ["kir", "source"]), kir = exact(site.kir, ["function_ordinal", "block_ordinal", "point"]);
  assert.equal(kir.function_ordinal, 0); integer(kir.block_ordinal);
  exact(kir.point, ["kind", "operation_ordinal"]); assert.equal(kir.point.kind, "operation"); integer(kir.point.operation_ordinal);
  exact(site.source, ["status", "location"]); assert.equal(site.source.status, "resolved");
  const location = exact(site.source.location, ["map_identity", "file_identity", "provenance", "byte_start", "byte_end"]);
  digest(location.map_identity); digest(location.file_identity); assert.equal(location.provenance, "compiler_bundle_bound");
  integer(location.byte_start); integer(location.byte_end, location.byte_start + 1);
  assert.ok(Array.isArray(snapshot.values) && snapshot.values.length > 0 && snapshot.values.length <= 64);
  const ssa = new Set();
  for (const row of snapshot.values) {
    exact(row, ["path", "availability"]); exact(row.path, ["root", "components"]);
    assert.deepEqual(row.path.components, []);
    const root = exact(row.path.root, ["kind", "function_ordinal", "frame", "value_ordinal"]);
    assert.equal(root.kind, "ssa"); assert.equal(root.function_ordinal, 0); assert.equal(root.frame, 1);
    integer(root.value_ordinal); assert.ok(!ssa.has(root.value_ordinal)); ssa.add(root.value_ordinal);
  }
  return snapshot;
}

/** Verify a closed actual assembly_chain checkpoint group, not arbitrary input admission.
 * Source-variable anchors are explicitly frame-refined, not equal to the unframed
 * control/stack/memory anchor. Both originals are retained without alteration. */
export function verifySourceValueCheckpoint({ control, stack, sourcePages, memory }, expectedBytes) {
  const snapshot = checkpoint(control), anchor = snapshot.anchor, stopped = control.response.session;
  unchanged(stack, "inspect_stack", stopped);
  exact(stack.request, ["schema", "request_id", "expected_revision", "operation", "scope", "page"]);
  assert.deepEqual(stack.request.scope, { level: "dispatch" }); assert.deepEqual(stack.request.page, { limit: 16 });
  const stackResult = exact(stack.response.result, ["result", "snapshot", "frames"]);
  assert.equal(stackResult.result, "stack"); assert.deepEqual(stackResult.snapshot, anchor);
  assert.ok(Array.isArray(stackResult.frames) && stackResult.frames.length === 1, "one real top-level frame required");
  const frame = stackResult.frames[0];
  exact(frame, ["frame", "function_ordinal", "block_ordinal", "next_operation", "values"]);
  assert.equal(frame.frame, 1); assert.equal(frame.function_ordinal, 0);
  assert.equal(frame.block_ordinal, anchor.site.kir.block_ordinal); integer(frame.next_operation);
  // An after-operation checkpoint's next operation is not its recorded site.
  assert.deepEqual(frame.values, { status: "captured", value_count: snapshot.values.length });
  assert.ok(Array.isArray(sourcePages) && sourcePages.length > 0 && sourcePages.length <= 32);
  const values = [], identities = new Set();
  let cursor, framedAnchor;
  for (const pair of sourcePages) {
    unchanged(pair, "inspect_source_variables", stopped, V2);
    exact(pair.request, ["schema", "request_id", "expected_revision", "operation", "scope", "frame", "selector", "page"]);
    assert.deepEqual(pair.request.scope, { level: "dispatch" }); assert.equal(pair.request.frame, frame.frame);
    assert.deepEqual(pair.request.selector, { selector: "all" });
    assert.deepEqual(pair.request.page, { limit: 2, ...(cursor ? { cursor } : {}) });
    const response = pair.response;
    exact(response, ["status", "schema", "request_id", "operation", "session", "snapshot", "values",
      ...(Object.hasOwn(response, "next_cursor") ? ["next_cursor"] : [])]);
    // Construct the explicit supported refinement only for comparison; retain the actual response anchor.
    assert.deepEqual(response.snapshot, { cursor: anchor.cursor, scope: anchor.scope, site: anchor.site,
      frame: frame.frame, occurrence: 1 }, "source query is the supported frame1 refinement of this exact stop");
    framedAnchor ??= response.snapshot;
    assert.deepEqual(response.snapshot, framedAnchor);
    assert.ok(Array.isArray(response.values) && response.values.length > 0 && response.values.length <= 2);
    for (const value of response.values) {
      exact(value, ["variable_identity", "name", "function_ordinal", "scope_identity", "scope_depth", "generation", "availability"]);
      digest(value.variable_identity); digest(value.scope_identity);
      assert.equal(value.function_ordinal, frame.function_ordinal); integer(value.scope_depth, 0, 4096);
      integer(value.generation); assert.equal(typeof value.name, "string");
      assert.ok(value.name.length > 0 && Buffer.byteLength(value.name) <= 4096 && !/[\u0000-\u001f\u007f]/u.test(value.name));
      assert.ok(!identities.has(value.variable_identity), "duplicate variable identity across pages"); identities.add(value.variable_identity);
      values.push(value);
    }
    cursor = response.next_cursor;
    if (cursor !== undefined) {
      exact(cursor, ["query_identity", "position"]); digest(cursor.query_identity);
      assert.equal(cursor.position, values.length); assert.equal(response.values.length, 2);
      if (sourcePages.indexOf(pair) > 0) assert.equal(cursor.query_identity, pair.request.page.cursor.query_identity);
    }
    assert.ok(values.length <= 64);
    assert.equal(cursor === undefined, pair === sourcePages.at(-1), "retain the complete ordered page chain");
  }
  for (const [name, bits] of [["a", "0xfffffff0"], ["b", "0x00000025"]]) {
    const rows = values.filter(value => value.name === name); assert.equal(rows.length, 1);
    assert.equal(rows[0].generation, 1);
    assert.deepEqual(rows[0].availability, { status: "value", value: { status: "captured",
      value_type: { kind: "integer", signed: false, bits: 32 }, value: { encoding: "bits", bits }, provenance: "simulated_observation" } });
  }
  for (const name of ["out", "result"]) {
    const rows = values.filter(value => value.name === name); assert.equal(rows.length, 1, `retained ${name} source variable`);
    assert.equal(rows[0].generation, 0);
    assert.deepEqual(rows[0].availability, { status: "value", value: { status: "unavailable", reason: "not_represented" } });
  }
  unchanged(memory, "read_memory", stopped);
  exact(memory.request, ["schema", "request_id", "expected_revision", "operation", "allocation", "byte_offset", "byte_len"]);
  const allocation = memory.request.allocation;
  assert.deepEqual(allocation, { ordinal: integer(allocation.ordinal, 1), generation: 0 });
  assert.equal(memory.request.byte_offset, 0); assert.equal(memory.request.byte_len, 24);
  assert.equal(memory.response.result.result, "memory"); assert.deepEqual(memory.response.result.snapshot, anchor);
  assert.deepEqual(memory.response.result.memory, { allocation, byte_offset: 0, requested_bytes: 24, returned_bytes: 24,
    availability: { status: "captured", address_space: "global", bytes: expectedBytes, initialized: "0xffffff", truncated: false } });
  const pairs = [control, stack, ...sourcePages, memory];
  for (let index = 1; index < pairs.length; index++) assert.ok(pairs[index].request.request_id > pairs[index - 1].request.request_id);
  return copy({ checkpoint_anchor: anchor, source_variable_anchor: framedAnchor,
    refinement: "same_stop_explicit_frame1_legacy_occurrence1_not_dynamic_activation",
    control_request_id: control.request.request_id, stack_request_id: stack.request.request_id,
    source_variable_request_ids: sourcePages.map(pair => pair.request.request_id), memory_request_id: memory.request.request_id,
    stack_frame: frame, values, ssa_values: snapshot.values, memory: memory.response.result.memory,
    source_to_ssa_mapping: "not_supplied", source_authentication: false, hardware_observed: false });
}

export function verifySourceValueReplay(groups, initialBytes) {
  assert.ok(Array.isArray(groups) && groups.length === 3);
  const [post, reverse, repeat] = groups.map(group => verifySourceValueCheckpoint(group, initialBytes));
  assert.deepEqual(groups.map(group => group.control.request.direction), ["forward", "reverse", "forward"]);
  assert.equal(reverse.checkpoint_anchor.cursor.state_revision, post.checkpoint_anchor.cursor.state_revision + 1);
  assert.equal(repeat.checkpoint_anchor.cursor.state_revision, reverse.checkpoint_anchor.cursor.state_revision + 1);
  assert.equal(groups[1].control.request.expected_revision, post.checkpoint_anchor.cursor.state_revision);
  assert.equal(groups[2].control.request.expected_revision, reverse.checkpoint_anchor.cursor.state_revision);
  assert.ok(reverse.checkpoint_anchor.cursor.event_sequence < post.checkpoint_anchor.cursor.event_sequence);
  assert.equal(post.checkpoint_anchor.cursor.event_sequence - reverse.checkpoint_anchor.cursor.event_sequence,
    groups[1].control.response.result.events_advanced);
  assert.equal(repeat.checkpoint_anchor.cursor.event_sequence - reverse.checkpoint_anchor.cursor.event_sequence,
    groups[2].control.response.result.events_advanced);
  assert.equal(repeat.checkpoint_anchor.cursor.event_sequence, post.checkpoint_anchor.cursor.event_sequence);
  for (const item of [reverse, repeat]) {
    assert.equal(item.checkpoint_anchor.cursor.configuration_identity, post.checkpoint_anchor.cursor.configuration_identity);
    assert.deepEqual(item.checkpoint_anchor.scope, post.checkpoint_anchor.scope);
    assert.deepEqual(item.values, post.values, "retained unchanged source variables survive actual reverse/repeat");
    assert.deepEqual(item.memory.allocation, post.memory.allocation);
  }
  assert.deepEqual(repeat.checkpoint_anchor.site, post.checkpoint_anchor.site);
  assert.deepEqual(repeat.ssa_values, post.ssa_values, "actual repeat restores the selected SSA state");
  assert.deepEqual(repeat.stack_frame, post.stack_frame);
  assert.deepEqual(repeat.memory, post.memory);
  return { observations: [post, reverse, repeat], forward_reverse_repeat: "observed_selected_retained_state_only" };
}

function regular(path, limit) {
  const before = lstatSync(path); assert.ok(before.isFile() && !before.isSymbolicLink() && before.size > 0 && before.size <= limit);
  const bytes = readFileSync(path), after = lstatSync(path);
  assert.equal(bytes.length, before.size);
  for (const field of ["dev", "ino", "size", "mtimeMs", "ctimeMs"]) assert.equal(after[field], before[field]);
  return bytes;
}

async function run(sourceDirectory, outputDirectory) {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), ".."), baseline = resolve(sourceDirectory), output = resolve(outputDirectory);
  mkdirSync(output); // A failed or prior run is never overwritten.
  const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
  const json = (name, value) => save(name, JSON.stringify(value, null, 2) + "\n");
  const pairs = [], requestLines = [], responseLines = [];
  let child, pending, timer, failed, exited, exitedValue, closing = false, revision = 0, nextId = 1;
  let stderr = "", buffered = "", totalBytes = 0;
  const fail = error => { failed ??= error; pending?.reject(error); pending = undefined; child?.kill("SIGKILL"); };
  const ask = async command => {
    if (failed) throw failed;
    assert.equal(pending, undefined); integer(nextId, 1, 256);
    const request = { schema: V1, request_id: nextId++, expected_revision: revision, ...command };
    const line = JSON.stringify(request) + "\n"; assert.ok(Buffer.byteLength(line) <= 65536); requestLines.push(line);
    const response = await new Promise((resolveReply, rejectReply) => {
      const deadline = setTimeout(() => fail(new Error("debugger response deadline")), 30000);
      pending = { resolve(value) { clearTimeout(deadline); resolveReply(value); }, reject(error) { clearTimeout(deadline); rejectReply(error); } };
      child.stdin.write(line, error => { if (error) fail(error); });
    });
    assert.equal(response.request_id, request.request_id); assert.equal(response.operation, request.operation);
    assert.equal(response.schema, expectedSchema(request.schema));
    revision = integer(response.session.revision); const pair = { request, response }; pairs.push(pair); return pair;
  };
  try {
    const receiptBytes = regular(join(baseline, "receipt.json"), 1024 * 1024), receipt = JSON.parse(receiptBytes);
    assert.equal(receipt.schema, "fe2o3-assembly-authoring-source-smoke-v30"); assert.equal(receipt.hardware_observed, false);
    const sourcePath = "crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30/src/lib.rs";
    assert.equal(receipt.source, sourcePath);
    const source = regular(join(root, sourcePath), 256 * 1024); assert.equal(hash(source), receipt.source_sha256);
    const bundlePath = join(baseline, "base-v6.fe2sim"), requestPath = join(baseline, "base-request.json");
    const bundle = regular(bundlePath, 64 * 1024 * 1024), requestBytes = regular(requestPath, 65536), request = JSON.parse(requestBytes);
    assert.equal(hash(bundle), receipt.base.bundle_sha256);
    const beforeBytes = "0x" + "a5a5a5a5".repeat(4) + "deadbeefcafebabe";
    assert.deepEqual(request, { schema: "fe2o3-simulation-request-v1", kernel: "assembly_chain", grid: [4, 1, 1], workgroup: [64, 1, 1],
      arguments: [{ kind: "buffer", element: "u32", access: "read_write", alignment: 4, bytes: beforeBytes },
        { kind: "scalar", type: "u32", bits: "0xfffffff0" }, { kind: "scalar", type: "u32", bits: "0x00000025" }] });
    const a = 0xfffffff0n, b = 0x25n, mask = 0xffffffffn;
    const expected = (((((a + b) & mask) - b) & mask) ^ b) & 255n | 256n;
    assert.equal(expected, 469n); const word = Buffer.alloc(4); word.writeUInt32LE(Number(expected));
    assert.equal(word.toString("hex"), "d5010000"); // Source smoke separately qualifies all four final words.
    const bin = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug");
    const toolPins = ["fe2o3-author", "fe2o3-debug"].map(name => {
      const path = join(bin, name), bytes = regular(path, 512 * 1024 * 1024); return { path, bytes: bytes.length, sha256: hash(bytes) };
    });
    const inspected = spawnSync(toolPins[0].path, ["inspect"], { cwd: root, input: bundle, encoding: "utf8", maxBuffer: 2 * 1024 * 1024, timeout: 30000 });
    save("admitted-inspection.stdout", inspected.stdout ?? ""); save("admitted-inspection.stderr", inspected.stderr ?? "");
    assert.equal(inspected.error, undefined); assert.equal(inspected.status, 0);
    const summary = JSON.parse(inspected.stdout);
    assert.equal(summary.bundle_identity, receipt.base.summary.bundle_identity); assert.equal(summary.canonical_kir_digest, receipt.base.summary.canonical_kir_digest);
    assert.equal(summary.canonical_kir_version, 11); assert.equal(summary.authority.grants_production_resume, false);
    child = spawn(toolPins[1].path, ["sim", "--bundle-v6", bundlePath, "--request", requestPath, "--wave-width", "32"],
      { cwd: root, stdio: ["pipe", "pipe", "pipe"] });
    exited = new Promise(resolveExit => child.once("close", (code, signal) => {
      exitedValue = { code, signal };
      if (pending) fail(new Error("debugger closed before the pending response drained"));
      resolveExit(exitedValue);
    }));
    timer = setTimeout(() => fail(new Error("debugger process deadline")), 120000);
    child.once("error", fail); child.stdin.on("error", fail);
    child.stdout.setEncoding("utf8"); child.stderr.setEncoding("utf8");
    child.stderr.on("data", chunk => { stderr += chunk; if (Buffer.byteLength(stderr) > 65536) fail(new Error("stderr byte bound")); });
    child.stdout.on("data", chunk => {
      try {
        totalBytes += Buffer.byteLength(chunk); buffered += chunk;
        assert.ok(totalBytes <= 16 * 1024 * 1024 && Buffer.byteLength(buffered) <= 2 * 1024 * 1024, "output byte budget");
        const newline = buffered.indexOf("\n"); if (newline < 0) return;
        assert.ok(pending, "unsolicited debugger response");
        const line = buffered.slice(0, newline + 1); buffered = buffered.slice(newline + 1);
        assert.equal(buffered, "", "one response per command");
        const value = JSON.parse(line); responseLines.push(line); const current = pending; pending = undefined; current.resolve(value);
      } catch (error) { fail(error); }
    });
    // exit may precede delivery of the final stdout response; close is the drain boundary.
    child.once("exit", (code, signal) => { if (!closing || code !== 0 || signal !== null) fail(new Error(`unexpected debugger exit ${code}/${signal}`)); });
    const step = async (direction, count = 1) => ask({ operation: "step", direction, granularity: "operation", count });
    const initial = await step("forward"); assert.equal(initial.response.result.snapshot.status, "captured");
    const inventory = await ask({ schema: "fe2o3-debug-resource-request-v1", operation: "query_allocations",
      expected_snapshot: initial.response.result.snapshot.snapshot.anchor, page: { max_items: 16, max_scanned: 16 } });
    assert.equal(inventory.response.status, "ok"); assert.equal(inventory.response.result.allocations.length, 1);
    const allocation = inventory.response.result.allocations[0].allocation;
    assert.deepEqual(allocation, { ordinal: integer(allocation.ordinal, 1), generation: 0 });
    const group = async direction => {
      // Each marker is a single-operation block: cross its after-operation
      // checkpoint to the next before-operation stop with an available frame.
      const control = await step(direction, 2), stack = await ask({ operation: "inspect_stack", scope: { level: "dispatch" }, page: { limit: 16 } });
      const sourcePages = []; let cursor;
      for (let index = 0; index < 32; index++) {
        const page = await ask({ schema: V2, operation: "inspect_source_variables", scope: { level: "dispatch" }, frame: 1,
          selector: { selector: "all" }, page: { limit: 2, ...(cursor ? { cursor } : {}) } });
        assert.equal(page.response.status, "ok"); sourcePages.push(page); cursor = page.response.next_cursor;
        if (cursor === undefined) break;
      }
      assert.equal(cursor, undefined, "source variable page budget");
      const memory = await ask({ operation: "read_memory", allocation, byte_offset: 0, byte_len: 24 });
      return { control, stack, sourcePages, memory };
    };
    const post = await group("forward");
    const readOnlyRefusal = async (command, status, code) => {
      const before = revision, stopped = copy(pairs.at(-1).response.session), pair = await ask(command);
      assert.equal(pair.response.status, status); assert.equal(revision, before);
      assert.deepEqual(pair.response.session, stopped, "refusal leaves the complete retained stop unchanged");
      assert.equal(status === "error" ? pair.response.error.code : pair.response.reason, code);
      if (status === "error") assert.equal(pair.response.error.state_changed, false);
      return pair;
    };
    const sourceRequest = { schema: V2, operation: "inspect_source_variables", scope: { level: "dispatch" }, frame: 1,
      selector: { selector: "all" }, page: { limit: 2 } };
    const oldCursor = post.sourcePages[0].response.next_cursor; assert.ok(oldCursor);
    await readOnlyRefusal({ ...sourceRequest, frame: 2 }, "unavailable", "frame_unavailable");
    await readOnlyRefusal({ ...sourceRequest, selector: { selector: "name", name: "a" }, page: { limit: 2, cursor: oldCursor } }, "error", "invalid_cursor");
    const reverse = await group("reverse");
    await readOnlyRefusal({ ...sourceRequest, expected_revision: post.control.response.session.revision }, "error", "stale_revision");
    await readOnlyRefusal({ ...sourceRequest, page: { limit: 2, cursor: oldCursor } }, "error", "invalid_cursor");
    const repeat = await group("forward"), groups = [post, reverse, repeat];
    const verified = verifySourceValueReplay(groups, beforeBytes);
    assert.equal(reverse.control.response.session.cursor.event_sequence, initial.response.session.cursor.event_sequence);
    assert.deepEqual(reverse.control.response.result.snapshot.snapshot.values, initial.response.result.snapshot.snapshot.values,
      "actual reverse restores independently retained entry SSA state");
    const retainedIds = new Set(groups.flatMap(value => [value.control, value.stack, ...value.sourcePages, value.memory]).map(pair => pair.request.request_id));
    closing = true; assert.equal((await ask({ operation: "terminate" })).response.status, "ok"); child.stdin.end();
    assert.deepEqual(await exited, { code: 0, signal: null }); clearTimeout(timer); if (failed) throw failed;
    assert.equal(buffered, ""); assert.equal(requestLines.length, responseLines.length);
    assert.equal(hash(regular(join(root, sourcePath), 256 * 1024)), receipt.source_sha256);
    assert.equal(hash(regular(bundlePath, 64 * 1024 * 1024)), hash(bundle));
    assert.equal(hash(regular(requestPath, 65536)), hash(requestBytes));
    for (const pin of toolPins) assert.equal(hash(regular(pin.path, 512 * 1024 * 1024)), pin.sha256);
    const excerptRequests = pairs.map((pair, index) => retainedIds.has(pair.request.request_id) ? requestLines[index] : "").join("");
    const excerptResponses = pairs.map((pair, index) => retainedIds.has(pair.request.request_id) ? responseLines[index] : "").join("");
    assert.ok(Buffer.byteLength(excerptRequests) <= 256 * 1024 && Buffer.byteLength(excerptResponses) <= 256 * 1024);
    for (const raw of [excerptRequests, excerptResponses]) for (const line of raw.trimEnd().split("\n")) assert.ok(Buffer.byteLength(line) + 1 <= 65536);
    save("debug-requests.jsonl", requestLines.join("")); save("debug-responses.jsonl", responseLines.join("")); save("debug-stderr.txt", stderr);
    save("resource-source-values.requests.jsonl", excerptRequests); save("resource-source-values.responses.jsonl", excerptResponses);
    json("observations.json", verified);
    json("receipt.json", { status: "passed", purpose: "existing-public-source-variable-resource-checkpoint-qualification",
      source: sourcePath, source_sha256: hash(source), source_smoke_receipt_sha256: hash(receiptBytes),
      bundle_sha256: hash(bundle), bundle_identity: summary.bundle_identity, canonical_kir_digest: summary.canonical_kir_digest,
      request_sha256: hash(requestBytes), tools: toolPins, checkpoints: verified.observations.map(value => ({
        control_request_id: value.control_request_id, source_variable_request_ids: value.source_variable_request_ids,
        checkpoint_anchor: value.checkpoint_anchor, source_variable_anchor: value.source_variable_anchor })),
      excerpts: [{ path: "resource-source-values.requests.jsonl", bytes: Buffer.byteLength(excerptRequests), sha256: hash(excerptRequests) },
        { path: "resource-source-values.responses.jsonl", bytes: Buffer.byteLength(excerptResponses), sha256: hash(excerptResponses) }],
      raw_line_preservation: true, source_variable_paging: "complete_limit2", refusal_count: 4,
      memory_observation: "initial_output_and_canary_bytes_unchanged_no_write_attribution",
      source_to_ssa_mapping: "not_supplied", dynamic_helper_activation: "not_represented", allocation_reuse: "not_represented",
      source_authentication: false, hardware_observed: false, performance_prediction: false });
    process.stdout.write(`Actual source-variable/resource checkpoint capture passed: ${output}\n`);
  } catch (error) {
    fail(error);
    if (exited && !exitedValue) await exited;
    clearTimeout(timer);
    json("failure.json", { status: "failed", detail: String(error).slice(0, 4096), child_exit: exitedValue ?? null, manufactured_fallback: false });
    save("partial-requests.jsonl", requestLines.join("")); save("partial-responses.jsonl", responseLines.join("")); save("failure-stderr.txt", stderr);
    throw error;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 4) throw new Error("usage: node scripts/resource-source-values-v2-smoke.mjs SOURCE_ASSEMBLY_SMOKE_DIRECTORY NEW_OUTPUT_DIRECTORY");
  await run(process.argv[2], process.argv[3]);
}
