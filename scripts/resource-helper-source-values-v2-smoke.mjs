#!/usr/bin/env node
// Qualification of existing public CPU debugger queries; no new wire schema.
// Importing this file exposes only pure checks and starts no child or file work.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { lstatSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

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
  assert.equal(pair.request.granularity, "operation"); integer(pair.request.count, 1, 128);
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
  integer(kir.function_ordinal); integer(kir.block_ordinal);
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
    assert.equal(root.kind, "ssa"); integer(root.function_ordinal); integer(root.frame, 1, 2);
    integer(root.value_ordinal); const key = [root.function_ordinal, root.frame, root.value_ordinal].join(":");
    assert.ok(!ssa.has(key)); ssa.add(key);
  }
  return snapshot;
}


/** Closed qualification of actual debug_helper checkpoints, not arbitrary producer admission.
 * Test callers may supply clearly synthetic protocol shapes; those are not source provenance. */
export function verifyHelperSourceCheckpoint({ control, stack, sourcePages, memory }, expectedBytes) {
  const snapshot = checkpoint(control), anchor = snapshot.anchor, stopped = control.response.session;
  unchanged(stack, "inspect_stack", stopped);
  exact(stack.request, ["schema", "request_id", "expected_revision", "operation", "scope", "page"]);
  assert.deepEqual(stack.request.scope, { level: "dispatch" }); assert.deepEqual(stack.request.page, { limit: 16 });
  exact(stack.response, ["status", "schema", "request_id", "operation", "session", "result"]);
  const result = exact(stack.response.result, ["result", "snapshot", "frames"]);
  assert.equal(result.result, "stack"); assert.deepEqual(result.snapshot, anchor);
  assert.ok(Array.isArray(result.frames) && result.frames.length === 2, "exact complete root/helper stack required");
  let count = 0;
  for (const [index, frame] of result.frames.entries()) {
    exact(frame, ["frame", "function_ordinal", "block_ordinal", "values",
      ...(Object.hasOwn(frame, "next_operation") ? ["next_operation"] : [])]);
    assert.equal(frame.frame, index + 1); integer(frame.function_ordinal); integer(frame.block_ordinal);
    if (Object.hasOwn(frame, "next_operation")) integer(frame.next_operation);
    exact(frame.values, ["status", "value_count"]); assert.equal(frame.values.status, "captured");
    integer(frame.values.value_count, 1, 64); count += frame.values.value_count;
    const rows = snapshot.values.filter(row => row.path.root.frame === frame.frame);
    assert.equal(rows.length, frame.values.value_count, "every stack frame's complete SSA count must match");
    for (const row of rows) assert.equal(row.path.root.function_ordinal, frame.function_ordinal);
  }
  assert.equal(count, snapshot.values.length); assert.ok(count <= 64);
  const frame = result.frames[1];
  assert.notEqual(frame.function_ordinal, result.frames[0].function_ordinal, "ordinary nonrecursive helper profile");
  assert.equal(frame.function_ordinal, anchor.site.kir.function_ordinal);
  assert.equal(frame.block_ordinal, anchor.site.kir.block_ordinal); integer(frame.next_operation);
  assert.ok(Array.isArray(sourcePages) && sourcePages.length >= 2 && sourcePages.length <= 32);
  const values = [], identities = new Set();
  let cursor, framedAnchor;
  for (const [index, pair] of sourcePages.entries()) {
    unchanged(pair, "inspect_source_variables", stopped, V2);
    exact(pair.request, ["schema", "request_id", "expected_revision", "operation", "scope", "frame", "selector", "page"]);
    assert.deepEqual(pair.request.scope, { level: "dispatch" }); assert.equal(pair.request.frame, 2);
    assert.deepEqual(pair.request.selector, { selector: "all" });
    assert.deepEqual(pair.request.page, { limit: 1, ...(cursor ? { cursor } : {}) });
    const response = exact(pair.response, ["status", "schema", "request_id", "operation", "session", "snapshot", "values",
      ...(Object.hasOwn(pair.response, "next_cursor") ? ["next_cursor"] : [])]);
    assert.deepEqual(response.snapshot, { cursor: anchor.cursor, scope: anchor.scope, site: anchor.site, frame: 2, occurrence: 1 },
      "explicit helper-frame refinement; not equality with the unframed checkpoint or dynamic activation");
    framedAnchor ??= response.snapshot; assert.deepEqual(response.snapshot, framedAnchor);
    assert.ok(Array.isArray(response.values) && response.values.length === 1);
    for (const value of response.values) {
      exact(value, ["variable_identity", "name", "function_ordinal", "scope_identity", "scope_depth", "generation", "availability"]);
      digest(value.variable_identity); digest(value.scope_identity); integer(value.scope_depth, 0, 4096);
      assert.equal(value.function_ordinal, frame.function_ordinal); integer(value.generation);
      assert.equal(typeof value.name, "string"); assert.ok(value.name.length > 0 && Buffer.byteLength(value.name) <= 4096 &&
        !/[\p{Cc}\p{Cs}]/u.test(value.name));
      assert.ok(!identities.has(value.variable_identity)); identities.add(value.variable_identity);
      // The closed ordinary fixture represents only its unchanged f32 parameter.
      // All other observed variables must remain explicitly unrepresented, never reconstructed.
      if (value.name === "value") {
        assert.equal(value.generation, 1);
        assert.deepEqual(value.availability, { status: "value", value: { status: "captured",
          value_type: { kind: "float", bits: 32 }, value: { encoding: "bits", bits: "0x3f800000" },
          provenance: "simulated_observation" } });
      } else {
        assert.equal(value.generation, 0);
        assert.deepEqual(value.availability, { status: "value", value: { status: "unavailable", reason: "not_represented" } });
      }
      values.push(value); assert.ok(values.length <= 64);
    }
    cursor = response.next_cursor;
    if (cursor !== undefined) {
      exact(cursor, ["query_identity", "position"]); digest(cursor.query_identity);
      assert.equal(cursor.position, values.length);
      if (index > 0) assert.equal(cursor.query_identity, pair.request.page.cursor.query_identity);
    }
    assert.equal(cursor === undefined, index === sourcePages.length - 1, "complete ordered source page chain required");
  }
  for (const name of ["value", "adjusted"]) assert.equal(values.filter(value => value.name === name).length, 1, "exact ordinary helper variable");
  unchanged(memory, "read_memory", stopped);
  exact(memory.request, ["schema", "request_id", "expected_revision", "operation", "allocation", "byte_offset", "byte_len"]);
  exact(memory.response, ["status", "schema", "request_id", "operation", "session", "result"]);
  exact(memory.response.result, ["result", "snapshot", "memory"]);
  const allocation = memory.request.allocation;
  assert.deepEqual(allocation, { ordinal: integer(allocation.ordinal, 1), generation: 0 });
  assert.equal(memory.request.byte_offset, 0); assert.equal(memory.request.byte_len, 24);
  assert.equal(memory.response.result.result, "memory"); assert.deepEqual(memory.response.result.snapshot, anchor);
  assert.deepEqual(memory.response.result.memory, { allocation, byte_offset: 0, requested_bytes: 24, returned_bytes: 24,
    availability: { status: "captured", address_space: "global", bytes: expectedBytes, initialized: "0xffffff", truncated: false } });
  const pairs = [control, stack, ...sourcePages, memory];
  for (let index = 1; index < pairs.length; index++) assert.ok(pairs[index].request.request_id > pairs[index - 1].request.request_id);
  return copy({ checkpoint_anchor: anchor, source_variable_anchor: framedAnchor,
    refinement: "same_stop_explicit_frame2_legacy_occurrence1_not_dynamic_activation",
    control_request_id: control.request.request_id, stack_request_id: stack.request.request_id,
    source_variable_request_ids: sourcePages.map(pair => pair.request.request_id), memory_request_id: memory.request.request_id,
    stack_frames: result.frames, selected_frame: 2, values, ssa_values: snapshot.values, memory: memory.response.result.memory,
    source_to_ssa_mapping: "not_supplied", source_authentication: false, hardware_observed: false });
}

export function verifyHelperSourceReplay(groups, initialBytes) {
  assert.ok(Array.isArray(groups) && groups.length === 3);
  const [post, reverse, repeat] = groups.map(group => verifyHelperSourceCheckpoint(group, initialBytes));
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
    assert.equal(item.stack_frames[1].function_ordinal, post.stack_frames[1].function_ordinal);
    assert.deepEqual(item.values, post.values, "observed unchanged parameter and unavailable locals survive reverse/repeat");
    assert.deepEqual(item.memory, post.memory);
  }
  assert.deepEqual(repeat.checkpoint_anchor.site, post.checkpoint_anchor.site);
  assert.deepEqual(repeat.ssa_values, post.ssa_values); assert.deepEqual(repeat.stack_frames, post.stack_frames);
  const frameRows = (observation, frame) => observation.ssa_values.filter(row => row.path.root.frame === frame);
  assert.deepEqual(frameRows(reverse, 1), frameRows(post, 1), "suspended caller SSA must remain unchanged inside this helper");
  assert.notDeepEqual(frameRows(reverse, 2), frameRows(post, 2), "require two distinguishable selected-helper SSA states, not caller-only changes");
  return { observations: [post, reverse, repeat], forward_reverse_repeat: "observed_selected_retained_state_only" };
}

/** Independent source-level f32 oracle; this checks CPU execution, not GPU hardware. */
export function verifyHelperResult(simulation) {
  assert.equal(simulation.status, "ok"); assert.equal(simulation.hardware_observed, false);
  assert.equal(simulation.counts.invocations_executed, 4);
  assert.equal(simulation.arguments.length, 2); assert.equal(simulation.arguments[0].bits, "0x3f800000");
  const sum = Math.fround(Math.fround(1) + Math.fround(1)), word = Buffer.alloc(4);
  word.writeFloatLE(sum); assert.equal(word.toString("hex"), "00000040");
  assert.equal(simulation.arguments[1].value.bytes, "0x" + word.toString("hex").repeat(4) + "deadbeefcafebabe");
  assert.equal(simulation.arguments[1].value.initialized, "0xffffff");
  return { arithmetic: "f32(1)+f32(1)=f32(2)", result_bits: "0x40000000", output_words: 4, tail_canary_unchanged: true,
    hardware_observed: false, performance_prediction: false };
}

function regular(path, limit) {
  const before = lstatSync(path); assert.ok(before.isFile() && !before.isSymbolicLink() && before.size > 0 && before.size <= limit);
  const bytes = readFileSync(path), after = lstatSync(path);
  assert.equal(bytes.length, before.size);
  for (const field of ["dev", "ino", "size", "mtimeMs", "ctimeMs"]) assert.equal(after[field], before[field]);
  return bytes;
}


async function run(repositoryDirectory, outputDirectory) {
  const root = resolve(repositoryDirectory), output = resolve(outputDirectory);
  mkdirSync(output); // Refuse to overwrite a prior success or failure.
  const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
  const json = (name, value) => save(name, JSON.stringify(value, null, 2) + "\n");
  const pairs = [], requestLines = [], responseLines = [];
  let child, pending, timer, failed, exited, exitedValue, closing = false, revision = 0, nextId = 1;
  let stderr = "", buffered = "", totalBytes = 0;
  const fail = error => { failed ??= error; pending?.reject(error); pending = undefined; child?.kill("SIGKILL"); };
  const ask = async command => {
    if (failed) throw failed;
    assert.equal(pending, undefined); integer(nextId, 1, 512);
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
    const fixture = "crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device";
    const sourcePath = fixture + "/src/lib.rs", manifestPath = fixture + "/Cargo.toml";
    const inputPins = [sourcePath, manifestPath, "Cargo.toml", "Cargo.lock"].map(path => {
      const bytes = regular(join(root, path), 2 * 1024 * 1024); return { path, bytes: bytes.length, sha256: hash(bytes) };
    });
    const bin = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug");
    const toolPins = ["fe2o3-export-sim", "fe2o3-author", "fe2o3-kir-sim", "fe2o3-debug"].map(name => {
      const path = join(bin, name), bytes = regular(path, 512 * 1024 * 1024); return { path, bytes: bytes.length, sha256: hash(bytes) };
    });
    const stage = (label, index, args, input, env = process.env) => {
      const result = spawnSync(toolPins[index].path, args, { cwd: root, input, env, encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024, timeout: 300000 });
      save(label + ".stdout", result.stdout ?? ""); save(label + ".stderr", result.stderr ?? "");
      assert.equal(result.error, undefined, label + " spawn/deadline"); assert.equal(result.status, 0, label + " exit");
      return result.stdout;
    };
    const bundlePath = join(output, "helper-v6.fe2sim"), requestPath = join(output, "helper-request.json");
    const environment = { ...process.env, CARGO_PROFILE_DEV_DEBUG: "1" };
    for (const name of ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS"]) delete environment[name];
    const exportArgs = ["--crate", "fe2o3_production_ranked_bounds_fixture", "--output", bundlePath,
      "--target", "gfx942", "--bundle-version", "6", "--target-dir", join(output, "export-target"), "--",
      "--manifest-path", join(root, manifestPath), "--package", "fe2o3-production-ranked-bounds-fixture",
      "--features", "debug_helper", "--lib", "--offline"];
    stage("export", 0, exportArgs, undefined, environment);
    const bundle = regular(bundlePath, 64 * 1024 * 1024), summary = JSON.parse(stage("inspect", 1, ["inspect"], bundle));
    digest(summary.bundle_identity); digest(summary.canonical_kir_digest); assert.equal(summary.canonical_kir_version, 11);
    assert.equal(summary.authority.grants_production_resume, false); assert.equal(summary.authority.source_authenticated, false);
    const beforeBytes = "0x" + "a5a5a5a5".repeat(4) + "deadbeefcafebabe";
    const request = { schema: "fe2o3-simulation-request-v1", kernel: "debug_helper", grid: [4, 1, 1], workgroup: [64, 1, 1],
      arguments: [{ kind: "scalar", type: "f32", bits: "0x3f800000" },
        { kind: "buffer", element: "f32", access: "read_write", alignment: 4, bytes: beforeBytes }] };
    json("helper-request.json", request); const requestBytes = regular(requestPath, 65536);
    // Result qualification is independent of debugger replay and must pass first.
    const simulationText = stage("simulate", 2, ["--bundle-v6", bundlePath, "--request", requestPath]);
    const independent = verifyHelperResult(JSON.parse(simulationText));
    json("independent-result.json", independent);
    child = spawn(toolPins[3].path, ["sim", "--bundle-v6", bundlePath, "--request", requestPath, "--wave-width", "32"],
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
    const stackAt = async control => {
      assert.equal(control.response.status, "ok");
      assert.equal(control.response.result.snapshot.status, "captured", "discovery requires real captured checkpoints");
      return ask({ operation: "inspect_stack", scope: { level: "dispatch" }, page: { limit: 16 } });
    };
    const isHelper = (control, stack) => {
      const anchor = control.response.result.snapshot.snapshot.anchor, frames = stack.response.result?.frames;
      return stack.response.status === "ok" && frames?.length === 2 && frames[0].frame === 1 && frames[1].frame === 2 &&
        Number.isSafeInteger(frames[1].next_operation) && frames[1].next_operation >= 0 &&
        frames[1].function_ordinal !== frames[0].function_ordinal && frames[1].function_ordinal === anchor.site.kir.function_ordinal &&
        frames[1].block_ordinal === anchor.site.kir.block_ordinal && anchor.site.source.status === "resolved" &&
        anchor.site.source.location.provenance === "compiler_bundle_bound";
    };
    let initial = await step("forward");
    const inventory = await ask({ schema: "fe2o3-debug-resource-request-v1", operation: "query_allocations",
      expected_snapshot: initial.response.result.snapshot.snapshot.anchor, page: { max_items: 16, max_scanned: 16 } });
    assert.equal(inventory.response.status, "ok"); assert.equal(inventory.response.result.allocations.length, 1);
    const allocation = inventory.response.result.allocations[0].allocation;
    assert.deepEqual(allocation, { ordinal: integer(allocation.ordinal, 1), generation: 0 });
    let initialStack = await stackAt(initial), found = isHelper(initial, initialStack);
    for (let scan = 0; !found && scan < 128; scan++) {
      initial = await step("forward"); initialStack = await stackAt(initial); found = isHelper(initial, initialStack);
    }
    assert.ok(found, "producer limitation: no exact queryable two-frame helper checkpoint within discovery bound");
    const collect = async (control, stack) => {
      assert.ok(isHelper(control, stack), "producer limitation: selected checkpoint is not the current two-frame helper");
      const sourcePages = []; let cursor;
      for (let page = 0; page < 32; page++) {
        const pair = await ask({ schema: V2, operation: "inspect_source_variables", scope: { level: "dispatch" }, frame: 2,
          selector: { selector: "all" }, page: { limit: 1, ...(cursor ? { cursor } : {}) } });
        assert.equal(pair.response.status, "ok", "producer limitation: helper variables unavailable at real checkpoint");
        sourcePages.push(pair); cursor = pair.response.next_cursor; if (cursor === undefined) break;
      }
      assert.equal(cursor, undefined, "source variable page budget");
      const memory = await ask({ operation: "read_memory", allocation, byte_offset: 0, byte_len: 24 });
      const group = { control, stack, sourcePages, memory }; verifyHelperSourceCheckpoint(group, beforeBytes); return group;
    };
    const entry = await collect(initial, initialStack);
    // Never skip out of this source invocation to another lane/call and imply
    // an unrepresented dynamic activation. One adjacent operation step must
    // leave a second queryable, distinct SSA state in the same helper.
    const postControl = await step("forward"), postStack = await stackAt(postControl);
    const post = await collect(postControl, postStack);
    assert.deepEqual(post.control.response.result.snapshot.snapshot.anchor.scope, initial.response.result.snapshot.snapshot.anchor.scope);
    assert.equal(postStack.response.result.frames[1].function_ordinal, initialStack.response.result.frames[1].function_ordinal);
    const refuse = async (command, status, code) => {
      const before = revision, stopped = copy(pairs.at(-1).response.session), pair = await ask(command);
      assert.equal(pair.response.status, status); assert.equal(revision, before); assert.deepEqual(pair.response.session, stopped);
      assert.equal(status === "error" ? pair.response.error.code : pair.response.reason, code);
      if (status === "error") assert.equal(pair.response.error.state_changed, false);
      return pair;
    };
    const sourceRequest = { schema: V2, operation: "inspect_source_variables", scope: { level: "dispatch" },
      frame: 2, selector: { selector: "all" }, page: { limit: 1 } };
    const oldCursor = post.sourcePages[0].response.next_cursor; assert.ok(oldCursor);
    const refusals = [];
    refusals.push(await refuse({ ...sourceRequest, frame: 3 }, "unavailable", "frame_unavailable"));
    // A suspended caller may have no next_operation field. The public query
    // then refuses before cursor validation; retain that exact outcome.
    const callerQueryable = Object.hasOwn(postStack.response.result.frames[0], "next_operation");
    refusals.push(await refuse({ ...sourceRequest, frame: 1, page: { limit: 1, cursor: oldCursor } },
      callerQueryable ? "error" : "unavailable", callerQueryable ? "invalid_cursor" : "checkpoint_not_captured"));
    refusals.push(await refuse({ ...sourceRequest, selector: { selector: "name", name: "value" },
      page: { limit: 1, cursor: oldCursor } }, "error", "invalid_cursor"));
    const reverseControl = await step("reverse"), reverse = await collect(reverseControl, await stackAt(reverseControl));
    refusals.push(await refuse({ ...sourceRequest, expected_revision: post.control.response.session.revision }, "error", "stale_revision"));
    refusals.push(await refuse({ ...sourceRequest, page: { limit: 1, cursor: oldCursor } }, "error", "invalid_cursor"));
    const repeatControl = await step("forward"), repeat = await collect(repeatControl, await stackAt(repeatControl));
    const groups = [post, reverse, repeat], verified = verifyHelperSourceReplay(groups, beforeBytes);
    assert.equal(reverseControl.response.session.cursor.event_sequence, initial.response.session.cursor.event_sequence);
    assert.deepEqual(reverseControl.response.result.snapshot.snapshot.values, initial.response.result.snapshot.snapshot.values);
    assert.deepEqual(reverse.stack.response.result.frames, entry.stack.response.result.frames);
    const retainedIds = new Set(groups.flatMap(group => [group.control, group.stack, ...group.sourcePages, group.memory]).map(pair => pair.request.request_id));
    closing = true; assert.equal((await ask({ operation: "terminate" })).response.status, "ok"); child.stdin.end();
    assert.deepEqual(await exited, { code: 0, signal: null }); clearTimeout(timer); if (failed) throw failed;
    assert.equal(buffered, ""); assert.equal(requestLines.length, responseLines.length);
    for (const pin of inputPins) assert.equal(hash(regular(join(root, pin.path), 2 * 1024 * 1024)), pin.sha256);
    for (const pin of toolPins) assert.equal(hash(regular(pin.path, 512 * 1024 * 1024)), pin.sha256);
    assert.equal(hash(regular(bundlePath, 64 * 1024 * 1024)), hash(bundle)); assert.equal(hash(regular(requestPath, 65536)), hash(requestBytes));
    const excerptRequests = pairs.map((pair, index) => retainedIds.has(pair.request.request_id) ? requestLines[index] : "").join("");
    const excerptResponses = pairs.map((pair, index) => retainedIds.has(pair.request.request_id) ? responseLines[index] : "").join("");
    assert.ok(Buffer.byteLength(excerptRequests) <= 256 * 1024 && Buffer.byteLength(excerptResponses) <= 256 * 1024);
    for (const raw of [excerptRequests, excerptResponses]) for (const line of raw.trimEnd().split("\n")) assert.ok(Buffer.byteLength(line) + 1 <= 65536);
    save("debug-requests.jsonl", requestLines.join("")); save("debug-responses.jsonl", responseLines.join("")); save("debug-stderr.txt", stderr);
    save("resource-helper-source-values.requests.jsonl", excerptRequests); save("resource-helper-source-values.responses.jsonl", excerptResponses);
    json("observations.json", verified);
    json("receipt.json", { status: "passed", purpose: "existing-public-ordinary-helper-source-variable-resource-qualification",
      source: sourcePath, source_sha256: inputPins[0].sha256, inputs: inputPins, export_args: exportArgs,
      bundle_sha256: hash(bundle), bundle_identity: summary.bundle_identity, canonical_kir_digest: summary.canonical_kir_digest,
      request_sha256: hash(requestBytes), independent_result: independent, independent_simulation_sha256: hash(simulationText), tools: toolPins,
      checkpoints: verified.observations.map(value => ({ control_request_id: value.control_request_id,
        source_variable_request_ids: value.source_variable_request_ids, checkpoint_anchor: value.checkpoint_anchor,
        source_variable_anchor: value.source_variable_anchor })),
      excerpts: [{ path: "resource-helper-source-values.requests.jsonl", bytes: Buffer.byteLength(excerptRequests), sha256: hash(excerptRequests) },
        { path: "resource-helper-source-values.responses.jsonl", bytes: Buffer.byteLength(excerptResponses), sha256: hash(excerptResponses) }],
      raw_line_preservation: true, source_variable_paging: "complete_limit1", refusal_request_ids: refusals.map(pair => pair.request.request_id),
      memory_observation: "initial_output_and_canary_bytes_unchanged_no_write_attribution",
      stack_profile: "exact_two_frames_selected_current_helper_frame2_all_frame_ssa",
      source_to_ssa_mapping: "not_supplied", dynamic_helper_activation: "not_represented", allocation_reuse: "not_represented",
      source_authentication: false, hardware_observed: false, performance_prediction: false });
    process.stdout.write("Actual helper source-variable/resource capture passed: " + output + "\n");
  } catch (error) {
    fail(error); if (exited && !exitedValue) await exited; clearTimeout(timer);
    json("failure.json", { status: "failed", detail: String(error).slice(0, 4096), child_exit: exitedValue ?? null, manufactured_fallback: false });
    save("partial-requests.jsonl", requestLines.join("")); save("partial-responses.jsonl", responseLines.join("")); save("failure-stderr.txt", stderr);
    throw error;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 4) throw new Error("usage: node scripts/resource-helper-source-values-v2-smoke.mjs REPOSITORY_DIRECTORY NEW_OUTPUT_DIRECTORY");
  await run(process.argv[2], process.argv[3]);
}
