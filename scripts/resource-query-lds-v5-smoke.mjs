#!/usr/bin/env node
// Actual, separately exported V5 input only. Metadata is inert cross-file
// consistency, not compiler authentication. No builds, source edits or fallback IR.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, constants, fstatSync, mkdirSync, openSync, readSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const SOURCE_PATH = "crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/src/lib.rs";
export const LIMITS = Object.freeze({ metadata_bytes: 65536, source_bytes: 1024 * 1024,
  bundle_bytes: 64 * 1024 * 1024, export_log_bytes: 8 * 1024 * 1024,
  commands: 4096, discovery_steps: 256, records: 131072, pages: 512,
  page_items: 256, page_scanned: 256, request_bytes: 65536,
  response_bytes: 2 * 1024 * 1024, transcript_bytes: 64 * 1024 * 1024,
  stderr_bytes: 65536, reply_timeout_ms: 30000, process_timeout_ms: 180000 });
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
export const decodeUtf8 = (bytes) => new TextDecoder("utf-8", { fatal: true }).decode(bytes);
// A process exit does not imply its stdout/stderr have drained. Retain all
// pending bytes and finalize streaming decoders only at the close event.
export const waitForProcessClose = (child) => new Promise((resolveClose) => {
  child.once("close", (code, signal) => resolveClose({ code, signal }));
});
const clone = (value) => JSON.parse(JSON.stringify(value));
const hex = /^[0-9a-f]{64}$/u;

export function readBounded(path, maximum) {
  assert.ok(Number.isSafeInteger(maximum) && maximum > 0 && maximum <= LIMITS.bundle_bytes);
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const stat = fstatSync(fd);
    assert.ok(stat.isFile(), "input must be a regular file");
    assert.ok(Number.isSafeInteger(stat.size) && stat.size <= maximum, "input file bound exceeded");
    // One spare byte detects growth without an unbounded readFile allocation.
    const bytes = Buffer.alloc(stat.size + 1);
    let used = 0;
    while (used < bytes.length) {
      const count = readSync(fd, bytes, used, bytes.length - used, null);
      if (count === 0) break;
      used += count;
    }
    assert.equal(used, stat.size, "input size changed while reading");
    return bytes.subarray(0, used);
  } finally { closeSync(fd); }
}

function exactKeys(value, keys) {
  assert.ok(value !== null && typeof value === "object" && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), "unexpected metadata fields");
}

export function validateExport(metadata, files) {
  exactKeys(metadata, ["schema", "source_path", "source_sha256", "bundle_sha256", "command",
    "target", "nightly", "checkout_head", "checkout_dirty", "compiler_closure_attestation"]);
  assert.equal(metadata.schema, "fe2o3-workgroup-u32-v5-export-observation-v1");
  assert.equal(metadata.source_path, SOURCE_PATH);
  assert.equal(metadata.target, "gfx942:xnack-");
  assert.equal(metadata.nightly, "nightly-2026-04-03");
  assert.match(metadata.checkout_head, /^[0-9a-f]{40}$/u);
  assert.equal(typeof metadata.checkout_dirty, "boolean");
  assert.equal(metadata.compiler_closure_attestation, "unavailable");
  exactKeys(metadata.command, ["program", "args", "exit_code", "stdout_sha256", "stderr_sha256"]);
  assert.equal(metadata.command.exit_code, 0);
  assert.equal(typeof metadata.command.program, "string");
  assert.ok(metadata.command.program.length <= 4096);
  assert.equal(basename(metadata.command.program), "fe2o3-export-sim");
  const args = metadata.command.args;
  assert.ok(Array.isArray(args) && args.length <= 64 && args.length > 0);
  assert.ok(args.every((item) => typeof item === "string" && item.length <= 4096 && !item.includes("\0")));
  const option = (name) => {
    assert.equal(args.filter((item) => item === name).length, 1, `one ${name} argument required`);
    return args[args.indexOf(name) + 1];
  };
  assert.equal(option("--crate"), "fe2o3_production_ranked_bounds_fixture");
  assert.equal(option("--bundle-version"), "5");
  assert.equal(option("--target"), "gfx942");
  assert.equal(option("--features"), "workgroup_reduce_u32");
  if (args.includes("--package")) assert.equal(option("--package"), "fe2o3-production-ranked-bounds-fixture");
  assert.equal(resolve(root, option("--manifest-path")), join(root, dirname(dirname(SOURCE_PATH)), "Cargo.toml"));
  assert.equal(basename(option("--output")), "kernel-v5.fe2sim");
  assert.ok(args.includes("--") && args.includes("--lib"));
  for (const [value, bytes] of [[metadata.source_sha256, files.source], [metadata.bundle_sha256, files.bundle],
    [metadata.command.stdout_sha256, files.stdout], [metadata.command.stderr_sha256, files.stderr]]) {
    assert.match(value, hex);
    assert.equal(sha256(bytes), value, "inert file digest mismatch");
  }
  assert.ok(files.source.length > 0 && files.bundle.length > 0);
  return metadata;
}

export function reductionProfile() {
  const word = (value) => { const bytes = Buffer.alloc(4); bytes.writeUInt32LE(value); return bytes.toString("hex"); };
  // Independent integer recurrence for the fixed 64-element reduction tree.
  let scratch = Array(64).fill(2);
  for (const offset of [32, 16, 8, 4, 2, 1]) {
    const previous = scratch;
    scratch = previous.map((value, index) => index < offset ? value + previous[index + offset] : value);
  }
  assert.equal(scratch[0], 64 * 2);
  const canary = "deadbeefcafebabe";
  const initial = `0x${"a5".repeat(256)}${canary}`;
  const first = `0x${word(128)}${"a5".repeat(252)}${canary}`;
  const final = `0x${word(128).repeat(64)}${canary}`;
  return { request: { schema: "fe2o3-simulation-request-v1", kernel: "workgroup_reduce_u32",
    grid: [64, 1, 1], workgroup: [64, 1, 1], arguments: [
      { kind: "scalar", type: "u32", bits: "0x00000002" },
      { kind: "buffer", element: "u32", access: "read_write", alignment: 4, bytes: initial },
    ] }, initial, first, final, scratch: `0x${scratch.map(word).join("")}`,
    scratch_words: scratch, global_initialized: `0x${"ff".repeat(33)}`,
    scratch_initialized: `0x${"ff".repeat(32)}` };
}

// set_watchpoints appends. Remove the exact prior ID, then independently list
// and bind the new spec/ID before accepting a stop from it.
export async function replaceWatchpoint(ask, previous, allocation, label) {
  if (previous !== undefined) {
    assert.ok(Number.isSafeInteger(previous) && previous > 0);
    await ask({ operation: "remove_watchpoints", watchpoint_ids: [previous] });
  }
  const spec = { client_label: label, enabled: true, allocation, byte_offset: 0, byte_len: 4,
    access: "write", timing: "after_commit" };
  await ask({ operation: "set_watchpoints", watchpoints: [spec] });
  const listed = await ask({ operation: "list_watchpoints", page: { limit: 2 } });
  assert.equal(listed.result.next_cursor, undefined);
  assert.equal(listed.result.watchpoints.length, 1);
  const selected = listed.result.watchpoints[0];
  assert.deepEqual(selected.spec, spec);
  assert.ok(Number.isSafeInteger(selected.watchpoint_id) && selected.watchpoint_id > 0);
  const response = await ask({ operation: "continue", max_events: LIMITS.records });
  assert.equal(response.result.stop.reason, "watchpoint"); assert.equal(response.result.stop.exact, true);
  assert.equal(response.result.stop.watchpoint_id, selected.watchpoint_id);
  return response;
}

export async function runLdsSmoke(inputDirectory, outputDirectory) {
  const input = resolve(inputDirectory), output = resolve(outputDirectory);
  const files = { source: readBounded(join(root, SOURCE_PATH), LIMITS.source_bytes),
    bundle: readBounded(join(input, "kernel-v5.fe2sim"), LIMITS.bundle_bytes),
    stdout: readBounded(join(input, "export-stdout.txt"), LIMITS.export_log_bytes),
    stderr: readBounded(join(input, "export-stderr.txt"), LIMITS.export_log_bytes) };
  const metadataBytes = readBounded(join(input, "export.json"), LIMITS.metadata_bytes);
  const metadata = validateExport(JSON.parse(decodeUtf8(metadataBytes)), files);
  mkdirSync(output); // No overwrite/reuse of a previous evidence directory.
  const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
  const saveJson = (name, value) => save(name, `${JSON.stringify(value, null, 2)}\n`);
  const bin = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug");
  const bundlePath = join(output, "kernel-v5.fe2sim"), requestPath = join(output, "simulation-request.json");
  const profile = reductionProfile();
  const requestBytes = `${JSON.stringify(profile.request, null, 2)}\n`;
  const requestLines = [], responseLines = [];
  let child, pending, failure, processTimer, closing = false, stderr = "", buffer = "", totalBytes = 0;
  let revision = 0, nextId = 1, configuration, activeWatchpoint;
  const fail = (error) => {
    failure ??= error; clearTimeout(processTimer); pending?.reject(error); pending = undefined;
    child?.kill("SIGKILL");
  };
  async function ask(fields) {
    if (failure) throw failure;
    assert.equal(pending, undefined);
    assert.ok(nextId <= LIMITS.commands, "command budget exceeded");
    const request = { schema: "fe2o3-debug-request-v1", request_id: nextId++, expected_revision: revision, ...fields };
    const line = `${JSON.stringify(request)}\n`;
    assert.ok(Buffer.byteLength(line) <= LIMITS.request_bytes);
    requestLines.push(line);
    const response = await new Promise((resolveResponse, reject) => {
      const timer = setTimeout(() => fail(new Error("debugger response timeout")), LIMITS.reply_timeout_ms);
      pending = { resolve(value) { clearTimeout(timer); resolveResponse(value); }, reject(error) { clearTimeout(timer); reject(error); } };
      child.stdin.write(line, (error) => { if (error) fail(error); });
    });
    assert.equal(response.request_id, request.request_id);
    assert.equal(response.operation, request.operation);
    assert.equal(response.schema, request.schema === "fe2o3-debug-resource-request-v1" ? "fe2o3-debug-resource-response-v1" : "fe2o3-debug-response-v1");
    assert.equal(response.session.backend, "cpu_kir_simulator");
    assert.equal(response.session.execution_kind, "cpu_kir_simulation");
    assert.equal(response.session.simulated, true);
    assert.equal(response.session.hardware_observed, false);
    assert.equal(response.session.performance_prediction, false);
    assert.ok(Number.isSafeInteger(response.session.revision));
    assert.equal(response.session.cursor.state_revision, response.session.revision);
    configuration ??= response.session.configuration_identity;
    assert.equal(response.session.configuration_identity, configuration);
    assert.equal(response.session.cursor.configuration_identity, configuration);
    revision = response.session.revision;
    return response;
  }
  async function ok(fields) {
    const response = await ask(fields);
    assert.equal(response.status, "ok", JSON.stringify(response.error));
    return response;
  }
  async function rejected(fields, code) {
    const before = revision, response = await ask(fields);
    assert.equal(response.status, "error"); assert.equal(response.error.code, code);
    assert.equal(response.error.state_changed, false); assert.equal(revision, before);
    return response;
  }
  async function step(direction) {
    const response = await ok({ operation: "step", direction, granularity: "operation", count: 1 });
    assert.equal(response.result.stop.exact, true);
    assert.equal(response.result.snapshot.status, "captured");
    const anchor = response.result.snapshot.snapshot.anchor;
    assert.deepEqual(anchor.cursor, response.session.cursor);
    assert.equal(anchor.scope.level, "lane");
    assert.equal(anchor.scope.interpretation, "logical_visualization");
    assert.equal(anchor.scope.wave_width, 32);
    assert.ok(Number.isSafeInteger(anchor.scope.active_mask));
    assert.deepEqual(anchor.scope.workgroup, [0, 0, 0]);
    return { anchor_response: response, anchor: clone(anchor) };
  }
  async function resource(operation, anchor, fields = {}) {
    const response = await ok({ schema: "fe2o3-debug-resource-request-v1", operation, expected_snapshot: anchor,
      page: { max_items: LIMITS.page_items, max_scanned: LIMITS.page_scanned }, ...fields });
    assert.deepEqual(response.snapshot, anchor);
    assert.equal(response.physical_registers, "not_represented");
    assert.equal(response.page.completeness.status, "complete");
    assert.ok(response.page.scanned <= (fields.page?.max_scanned ?? LIMITS.page_scanned));
    assert.ok(Number.isSafeInteger(response.page.source_count) && response.page.source_count <= LIMITS.records);
    return response;
  }
  async function inventory(anchor) {
    const response = await resource("query_allocations", anchor);
    assert.equal(response.result.result, "allocations");
    assert.equal(response.page.next_token, undefined);
    assert.ok(response.result.allocations.length >= 1 && response.result.allocations.length <= 2);
    for (const row of response.result.allocations) {
      assert.equal(row.allocation.generation, 0);
      assert.equal(row.owning_scope, "not_represented"); assert.equal(row.lifetime, "not_represented");
      assert.equal(row.physical_base, "not_represented");
      assert.equal(row.snapshot_bytes_available, true); assert.equal(row.initialization_available, true);
      assert.equal(row.access, "read_write"); assert.equal(row.alignment, 4);
      assert.ok(["global", "workgroup"].includes(row.address_space));
      assert.equal(row.capacity_bytes, row.address_space === "global" ? "264" : "256");
    }
    return response;
  }
  async function memory(anchor, allocation, length, expectedBytes, expectedInitialization) {
    const response = await ok({ operation: "read_memory", allocation, byte_offset: 0, byte_len: length });
    assert.deepEqual(response.result.snapshot, anchor);
    assert.deepEqual(response.result.memory.allocation, allocation);
    const captured = response.result.memory.availability;
    assert.equal(captured.status, "captured"); assert.equal(captured.truncated, false);
    assert.match(captured.bytes, new RegExp(`^0x[0-9a-f]{${length * 2}}$`, "u"));
    if (expectedBytes !== undefined) assert.equal(captured.bytes, expectedBytes);
    assert.equal(captured.initialized, expectedInitialization);
    return response;
  }
  async function accesses(anchor, addressSpace, allocation) {
    const rows = [], pageRequestIds = [], tokens = new Set();
    let token, sourceCount, scanned = 0, previous = 0;
    for (let index = 0; index < LIMITS.pages; index++) {
      const response = await resource("query_memory_accesses", anchor, {
        filter: { scope: { level: "workgroup", workgroup: [0, 0, 0] }, address_space: addressSpace, allocation },
        page: { max_items: LIMITS.page_items, max_scanned: LIMITS.page_scanned, ...(token ? { token } : {}) },
      });
      assert.equal(response.result.result, "memory_accesses");
      assert.ok(response.result.accesses.length <= LIMITS.page_items);
      pageRequestIds.push(response.request_id);
      sourceCount ??= response.page.source_count;
      assert.equal(response.page.source_count, sourceCount); scanned += response.page.scanned;
      for (const row of response.result.accesses) {
        assert.ok(Number.isSafeInteger(row.occurrence.event_sequence));
        assert.ok(row.occurrence.event_sequence > previous && row.occurrence.event_sequence <= anchor.cursor.event_sequence);
        assert.equal(row.occurrence.event_sequence, row.occurrence.record_ordinal + 1);
        previous = row.occurrence.event_sequence;
        assert.deepEqual(row.allocation, allocation); assert.equal(row.address_space, addressSpace);
        assert.deepEqual(row.occurrence.scope.workgroup, [0, 0, 0]);
        assert.equal(row.occurrence.scope.wave_width, 32);
        assert.ok(Number.isSafeInteger(row.occurrence.scope.active_mask));
        assert.equal(row.occurrence.schedule.identity, "workgroup_major_local_zyx_cooperative_v1");
        assert.equal(row.source_association, "not_represented");
        assert.equal(row.call_frame, "not_represented"); assert.equal(row.operation_occurrence, "not_represented");
        assert.equal(row.range.byte_len, "4");
        assert.match(row.range.byte_offset, /^(0|[1-9][0-9]*)$/u);
        assert.ok(Number(row.range.byte_offset) % 4 === 0 && Number(row.range.byte_offset) < 256);
        assert.ok(["read", "write_committed"].includes(row.access));
        rows.push(row);
      }
      token = response.page.next_token;
      if (token === undefined) {
        assert.equal(scanned, sourceCount);
        return { rows, page_request_ids: pageRequestIds, raw_records_scanned: scanned };
      }
      assert.equal(typeof token, "string"); assert.ok(!tokens.has(token)); tokens.add(token);
    }
    throw new Error("access page budget exhausted");
  }
  async function watch(allocation, label) {
    const response = await replaceWatchpoint(ok, activeWatchpoint, allocation, label);
    activeWatchpoint = response.result.stop.watchpoint_id;
    return response;
  }

  try {
    save("source.rs", files.source); save("kernel-v5.fe2sim", files.bundle);
    save("export.json", metadataBytes); save("export-stdout.txt", files.stdout); save("export-stderr.txt", files.stderr);
    save("simulation-request.json", requestBytes);
    const simulation = spawnSync(join(bin, "fe2o3-kir-sim"), ["--bundle-v5", bundlePath, "--request", requestPath],
      { cwd: root, maxBuffer: 2 * 1024 * 1024, timeout: LIMITS.process_timeout_ms });
    save("simulation-stdout.json", simulation.stdout ?? ""); save("simulation-stderr.txt", simulation.stderr ?? "");
    assert.equal(simulation.error, undefined); assert.equal(simulation.status, 0, simulation.stderr);
    const result = JSON.parse(decodeUtf8(simulation.stdout));
    assert.equal(result.status, "ok"); assert.equal(result.hardware_observed, false);
    assert.equal(result.counts.invocations_executed, 64);
    assert.equal(result.arguments[0].bits, "0x00000002");
    assert.equal(result.arguments[1].value.bytes, profile.final);
    assert.equal(result.arguments[1].value.initialized, profile.global_initialized);

    // Wave32 is a losslessly representable logical view, not AMD hardware wave32.
    child = spawn(join(bin, "fe2o3-debug"), ["sim", "--bundle-v5", bundlePath, "--request", requestPath,
      "--wave-width", "32"], { cwd: root, stdio: ["pipe", "pipe", "pipe"] });
    processTimer = setTimeout(() => fail(new Error("debugger process timeout")), LIMITS.process_timeout_ms);
    const exited = waitForProcessClose(child);
    const stdoutDecoder = new TextDecoder("utf-8", { fatal: true }), stderrDecoder = new TextDecoder("utf-8", { fatal: true });
    child.once("error", fail);
    child.stderr.on("data", (chunk) => {
      try { stderr += stderrDecoder.decode(chunk, { stream: true }); assert.ok(Buffer.byteLength(stderr) <= LIMITS.stderr_bytes, "stderr bound exceeded"); }
      catch (error) { fail(error); }
    });
    child.stdout.on("data", (chunk) => {
      try {
        totalBytes += chunk.length; buffer += stdoutDecoder.decode(chunk, { stream: true });
        assert.ok(totalBytes <= LIMITS.transcript_bytes, "transcript byte budget exceeded");
        assert.ok(Buffer.byteLength(buffer) <= LIMITS.response_bytes, "response line bound exceeded");
        const newline = buffer.indexOf("\n"); if (newline < 0) return;
        assert.ok(pending, "unsolicited debugger response");
        const line = buffer.slice(0, newline + 1), response = JSON.parse(line);
        buffer = buffer.slice(newline + 1); assert.equal(buffer, "", "multiple responses for one request");
        responseLines.push(line); const completed = pending; pending = undefined; completed.resolve(response);
      } catch (error) { fail(error); }
    });
    child.once("close", (code, signal) => {
      try { assert.equal(stdoutDecoder.decode(), ""); assert.equal(stderrDecoder.decode(), ""); }
      catch (error) { fail(error); }
      if (!closing || pending || code !== 0) fail(new Error(`unexpected debugger exit ${code}/${signal}: ${stderr}`));
    });

    let pre, initialInventory, ldsRow, globalRow;
    for (let index = 0; index < LIMITS.discovery_steps; index++) {
      pre = await step("forward"); initialInventory = await inventory(pre.anchor);
      ldsRow = initialInventory.result.allocations.find((row) => row.address_space === "workgroup");
      if (ldsRow !== undefined) break;
    }
    assert.ok(ldsRow, "no live LDS allocation within bounded discovery");
    globalRow = initialInventory.result.allocations.find((row) => row.address_space === "global");
    assert.ok(globalRow); assert.notDeepEqual(globalRow.allocation, ldsRow.allocation);
    const lds = ldsRow.allocation, global = globalRow.allocation;
    const preLds = await memory(pre.anchor, lds, 256, undefined, `0x${"00".repeat(32)}`);
    const preGlobal = await memory(pre.anchor, global, 264, profile.initial, profile.global_initialized);
    const preAccesses = await accesses(pre.anchor, "workgroup", lds);
    assert.equal(preAccesses.rows.length, 0);
    const initialLdsBytes = preLds.result.memory.availability.bytes;
    const firstLdsBytes = `0x02000000${initialLdsBytes.slice(10)}`;
    const firstStop = await watch(lds, "first-lds-write");
    const first = await step("forward"), firstInventory = await inventory(first.anchor);
    const firstLds = await memory(first.anchor, lds, 256, firstLdsBytes, `0x0f${"00".repeat(31)}`);
    const firstAccesses = await accesses(first.anchor, "workgroup", lds);
    assert.equal(firstAccesses.rows.length, 1); assert.equal(firstAccesses.rows[0].access, "write_committed");
    assert.equal(firstAccesses.rows[0].range.byte_offset, "0");
    const tokenFilter = { scope: { level: "workgroup", workgroup: [0, 0, 0] }, address_space: "workgroup", allocation: lds };
    const tokenPage = await resource("query_memory_accesses", first.anchor, { filter: tokenFilter, page: { max_items: 1, max_scanned: 1 } });
    assert.equal(typeof tokenPage.page.next_token, "string");
    const reverse = await step("reverse");
    const reverseLds = await memory(reverse.anchor, lds, 256, initialLdsBytes, `0x${"00".repeat(32)}`);
    const reverseAccesses = await accesses(reverse.anchor, "workgroup", lds);
    assert.equal(reverseAccesses.rows.length, 0, "future committed accesses must remain hidden");
    const again = await step("forward");
    assert.equal(again.anchor.cursor.event_sequence, first.anchor.cursor.event_sequence);
    assert.notEqual(again.anchor.cursor.state_revision, first.anchor.cursor.state_revision);
    await rejected({ schema: "fe2o3-debug-resource-request-v1", operation: "query_allocations",
      expected_revision: first.anchor.cursor.state_revision, expected_snapshot: first.anchor,
      page: { max_items: 1, max_scanned: 1 } }, "stale_revision");
    await rejected({ schema: "fe2o3-debug-resource-request-v1", operation: "query_memory_accesses", expected_snapshot: again.anchor,
      filter: tokenFilter, page: { max_items: 1, max_scanned: 1, token: tokenPage.page.next_token } }, "invalid_cursor");
    await memory(again.anchor, lds, 256, firstLdsBytes, `0x0f${"00".repeat(31)}`);

    const reductionStop = await watch(global, "first-global-write-after-reduction");
    const reduction = await step("forward"), reductionInventory = await inventory(reduction.anchor);
    assert.ok(reductionInventory.result.allocations.some((row) => row.address_space === "workgroup"));
    const reductionLds = await memory(reduction.anchor, lds, 256, profile.scratch, profile.scratch_initialized);
    const reductionGlobal = await memory(reduction.anchor, global, 264, profile.first, profile.global_initialized);
    const reductionLdsAccesses = await accesses(reduction.anchor, "workgroup", lds);
    assert.equal(reductionLdsAccesses.rows.length, 1280);
    assert.equal(reductionLdsAccesses.rows.filter((row) => row.access === "write_committed").length, 448);
    assert.equal(reductionLdsAccesses.rows.filter((row) => row.access === "read").length, 832);
    const reductionGlobalAccesses = await accesses(reduction.anchor, "global", global);
    assert.equal(reductionGlobalAccesses.rows.length, 1);
    assert.equal(reductionGlobalAccesses.rows[0].range.byte_offset, "0");
    await ok({ operation: "remove_watchpoints", watchpoint_ids: [activeWatchpoint] });
    const completed = await ok({ operation: "continue", max_events: LIMITS.records });
    assert.equal(completed.result.stop.reason, "completed"); assert.equal(completed.result.stop.outcome, "completed");
    assert.equal(completed.result.stop.exact, true);
    const final = await step("reverse"), finalInventory = await inventory(final.anchor);
    const finalGlobal = await memory(final.anchor, global, 264, profile.final, profile.global_initialized);
    const finalGlobalAccesses = await accesses(final.anchor, "global", global);
    assert.deepEqual(finalGlobalAccesses.rows.map((row) => row.range.byte_offset), Array.from({ length: 64 }, (_, index) => String(index * 4)));
    assert.ok(finalGlobalAccesses.rows.every((row) => row.access === "write_committed"));
    closing = true; await ok({ operation: "terminate" }); child.stdin.end();
    assert.deepEqual(await exited, { code: 0, signal: null }); clearTimeout(processTimer);
    if (failure) throw failure;
    assert.equal(sha256(readBounded(join(root, SOURCE_PATH), LIMITS.source_bytes)), metadata.source_sha256);
    const requestText = requestLines.join(""), responseText = responseLines.join("");
    save("debug-requests.jsonl", requestText); save("debug-responses.jsonl", responseText); save("debug-stderr.txt", stderr);
    saveJson("resource-query-results.json", {
      schema: "fe2o3-lds-resource-query-checkpoints-v1",
      pre_write: { ...pre, inventory: initialInventory, memories: { workgroup: preLds, global: preGlobal }, accesses: { workgroup: preAccesses } },
      first_write: { ...first, watchpoint_stop: firstStop, inventory: firstInventory, memories: { workgroup: firstLds }, accesses: { workgroup: firstAccesses } },
      reverse_prewrite: { ...reverse, memories: { workgroup: reverseLds }, accesses: { workgroup: reverseAccesses } },
      forward_again: again,
      reduction: { ...reduction, watchpoint_stop: reductionStop, inventory: reductionInventory,
        memories: { workgroup: reductionLds, global: reductionGlobal }, accesses: { workgroup: reductionLdsAccesses, global: reductionGlobalAccesses } },
      final: { ...final, inventory: finalInventory, memories: { global: finalGlobal }, accesses: { global: finalGlobalAccesses },
        interpretation: "last captured operation checkpoint after navigating to completion; not a post-release lifetime observation" },
      completion_response: completed,
    });
    saveJson("receipt.json", { schema: "fe2o3-lds-resource-query-source-smoke-v1",
      source_path: SOURCE_PATH, source_sha256: metadata.source_sha256, bundle_sha256: metadata.bundle_sha256,
      export_observation_sha256: sha256(metadataBytes), simulation_request_sha256: sha256(requestBytes),
      simulation_stdout_sha256: sha256(simulation.stdout), debug_requests_sha256: sha256(requestText),
      debug_responses_sha256: sha256(responseText), commands: requestLines.length, response_bytes: totalBytes,
      expected_u32: 128, workgroup: [64, 1, 1], logical_wave_width: 32,
      workgroup_allocation: lds, global_allocation: global, workgroup_accesses_at_reduction: 1280,
      evidence_kind: "actual_v5_admission_and_cpu_simulation_observation",
      metadata_authority: "inert_cross_file_consistency_not_compiler_authentication",
      compiler_closure_attestation: "unavailable", source_edited: false, hardware_observed: false,
      performance_prediction: false, grants_production_resume: false, grants_load_authority: false, grants_launch_authority: false,
      allocation_generation: "producer_profile_zero_not_lifetime_evidence",
      owning_scope: "not_represented", lifetime: "not_represented", physical_base: "not_represented",
      physical_registers: "not_represented", access_source_association: "not_represented",
      checks: ["separate_actual_v5_cpu_execution", "exact_independent_checkpoint_anchors", "live_workgroup_allocation",
        "first_lds_write_bytes_and_initialization", "reverse_prewrite_no_future_access", "same_cursor_new_revision",
        "stale_anchor_and_token_refused", "complete_reduction_tree_bytes", "bounded_access_pages",
        "64_output_words_and_canaries", "source_unchanged"] });
    console.log(`Actual-source V5 LDS resource queries passed; evidence: ${output}`);
  } catch (error) {
    fail(error);
    saveJson("failure.json", { status: "failed", message: String(error), manufactured_capture_fallback: false, hardware_observed: false });
    save("partial-debug-requests.jsonl", requestLines.join("")); save("partial-debug-responses.jsonl", responseLines.join(""));
    save("failure-stderr.txt", stderr); throw error;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert.equal(process.argv.length, 4,
    "usage: node scripts/resource-query-lds-v5-smoke.mjs EXPORTED_SOURCE_DIRECTORY NEW_OUTPUT_DIRECTORY");
  await runLdsSmoke(process.argv[2], process.argv[3]);
}
