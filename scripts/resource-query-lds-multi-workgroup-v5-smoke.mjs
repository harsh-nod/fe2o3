#!/usr/bin/env node
// Same actually exported V5 source, two CPU workgroups. This observes snapshots
// and access records; it does not manufacture ownership, lifetime or generation.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, realpathSync, statfsSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const LIMITS = Object.freeze({ records: 65536, pages: 256, page_items: 256,
  page_scanned: 256, commands: 4096, discovery_steps: 256, request_bytes: 65536,
  response_bytes: 2 * 1024 * 1024, transcript_bytes: 64 * 1024 * 1024,
  result_bytes: 16 * 1024 * 1024, stderr_bytes: 65536,
  reply_timeout_ms: 30000, process_timeout_ms: 120000 });
const DISK_RESERVE = 40n * 1024n * 1024n * 1024n;
function diskReserve(path, bytes = 0) {
  const stat = statfsSync(path, { bigint: true });
  assert.ok(stat.bavail * stat.bsize >= DISK_RESERVE + BigInt(bytes), "40 GiB disk reserve required");
}
const clone = (value) => JSON.parse(JSON.stringify(value));
const safe = (value) => Number.isSafeInteger(value) && value >= 0;
const same = (a, b) => { assert.deepEqual(a, b); };
const within = (parent, child) => {
  const path = relative(parent, child);
  return path === "" || (!path.startsWith(`..${process.platform === "win32" ? "\\" : "/"}`) && path !== ".." && !isAbsolute(path));
};

export function twoWorkgroupProfile() {
  const word = (value) => { const bytes = Buffer.alloc(4); bytes.writeUInt32LE(value); return bytes.toString("hex"); };
  let tree = Array(64).fill(2);
  for (const distance of [32, 16, 8, 4, 2, 1]) {
    const prior = tree;
    tree = prior.map((value, index) => index < distance ? value + prior[index + distance] : value);
  }
  const canaries = "deadbeefcafebabe";
  const afterWords = (count) => {
    assert.ok(safe(count) && count <= 128);
    return `0x${word(128).repeat(count)}${"a5".repeat((128 - count) * 4)}${canaries}`;
  };
  return { request: { schema: "fe2o3-simulation-request-v1", kernel: "workgroup_reduce_u32",
    grid: [128, 1, 1], workgroup: [64, 1, 1], arguments: [
      { kind: "scalar", type: "u32", bits: "0x00000002" },
      { kind: "buffer", element: "u32", access: "read_write", alignment: 4, bytes: afterWords(0) },
    ] }, afterWords, initial: afterWords(0), final: afterWords(128),
    scratch: `0x${tree.map(word).join("")}`, scratch_words: tree,
    global_initialized: `0x${"ff".repeat(65)}`, scratch_initialized: `0x${"ff".repeat(32)}`,
    uninitialized_scratch: `0x${"00".repeat(32)}` };
}

export function validateAllocationRows(rows) {
  assert.ok(Array.isArray(rows) && rows.length >= 1 && rows.length <= 2);
  const ids = new Set(), spaces = new Set();
  for (const row of rows) {
    assert.ok(safe(row.allocation.ordinal) && row.allocation.ordinal > 0);
    assert.equal(row.allocation.generation, 0);
    assert.ok(!ids.has(row.allocation.ordinal)); ids.add(row.allocation.ordinal);
    assert.ok(["global", "workgroup"].includes(row.address_space));
    assert.ok(!spaces.has(row.address_space)); spaces.add(row.address_space);
    assert.equal(row.capacity_bytes, row.address_space === "global" ? "520" : "256");
    assert.equal(row.access, "read_write"); assert.equal(row.alignment, 4);
    assert.equal(row.snapshot_bytes_available, true); assert.equal(row.initialization_available, true);
    for (const field of ["owning_scope", "lifetime", "physical_base"]) assert.equal(row[field], "not_represented");
  }
  assert.ok(spaces.has("global"));
  return { global: rows.find((row) => row.address_space === "global").allocation,
    workgroup: rows.find((row) => row.address_space === "workgroup")?.allocation };
}

export function validateCheckpoint(response) {
  assert.equal(response.status, "ok"); assert.equal(response.result.stop.exact, true);
  assert.equal(response.result.snapshot.status, "captured");
  const anchor = response.result.snapshot.snapshot.anchor;
  same(anchor.cursor, response.session.cursor);
  assert.ok(safe(anchor.cursor.event_sequence) && anchor.cursor.event_sequence > 0);
  assert.ok(anchor.cursor.event_sequence <= LIMITS.records);
  assert.equal(anchor.scope.level, "lane"); assert.equal(anchor.scope.interpretation, "logical_visualization");
  assert.equal(anchor.scope.wave_width, 32); assert.ok(safe(anchor.scope.active_mask));
  assert.ok([0, 1].includes(anchor.scope.workgroup[0])); same(anchor.scope.workgroup.slice(1), [0, 0]);
  return clone(anchor);
}

export function sameCheckpointLocation(current, previous) {
  const expected = clone(previous);
  assert.ok(safe(current.cursor.state_revision));
  assert.notEqual(current.cursor.state_revision, previous.cursor.state_revision);
  expected.cursor.state_revision = current.cursor.state_revision;
  same(current, expected);
}

export function validateAccessRow(row, anchor, workgroup, addressSpace, allocation, previous) {
  assert.ok(safe(workgroup) && workgroup <= 1);
  const sequence = row.occurrence.event_sequence;
  assert.ok(safe(sequence) && sequence > previous && sequence <= anchor.cursor.event_sequence);
  assert.ok(safe(row.occurrence.record_ordinal)); assert.equal(sequence, row.occurrence.record_ordinal + 1);
  same(row.allocation, allocation); assert.equal(row.address_space, addressSpace);
  same(row.occurrence.scope.workgroup, [workgroup, 0, 0]);
  assert.equal(row.occurrence.scope.wave_width, 32); assert.ok(safe(row.occurrence.scope.active_mask));
  assert.equal(row.occurrence.scope.interpretation, "logical_visualization");
  assert.equal(row.occurrence.schedule.identity, "workgroup_major_local_zyx_cooperative_v1");
  for (const field of ["source_association", "call_frame", "operation_occurrence"]) assert.equal(row[field], "not_represented");
  assert.equal(row.range.byte_len, "4"); assert.match(row.range.byte_offset, /^(0|[1-9][0-9]*)$/u);
  const offset = Number(row.range.byte_offset);
  assert.ok(safe(offset) && offset % 4 === 0);
  if (addressSpace === "workgroup") assert.ok(offset < 256);
  else { assert.equal(addressSpace, "global"); assert.ok(offset >= workgroup * 256 && offset < (workgroup + 1) * 256); }
  assert.ok(["read", "write_committed"].includes(row.access));
  return sequence;
}

export async function replaceWatchpoint(ask, previous, allocation, byteOffset, label) {
  assert.ok(safe(byteOffset) && byteOffset % 4 === 0 && byteOffset <= 508);
  assert.ok(safe(allocation.ordinal) && allocation.ordinal > 0 && allocation.generation === 0);
  assert.ok(typeof label === "string" && label.length > 0 && label.length <= 64);
  if (previous !== undefined) {
    assert.ok(safe(previous) && previous > 0);
    await ask({ operation: "remove_watchpoints", watchpoint_ids: [previous] });
  }
  const spec = { client_label: label, enabled: true, allocation, byte_offset: byteOffset, byte_len: 4,
    access: "write", timing: "after_commit" };
  await ask({ operation: "set_watchpoints", watchpoints: [spec] });
  const listed = await ask({ operation: "list_watchpoints", page: { limit: 2 } });
  assert.equal(listed.result.next_cursor, undefined); assert.equal(listed.result.watchpoints.length, 1);
  const selected = listed.result.watchpoints[0]; same(selected.spec, spec);
  assert.ok(safe(selected.watchpoint_id) && selected.watchpoint_id > 0);
  const response = await ask({ operation: "continue", max_events: LIMITS.records });
  assert.equal(response.result.stop.reason, "watchpoint"); assert.equal(response.result.stop.exact, true);
  assert.equal(response.result.stop.watchpoint_id, selected.watchpoint_id);
  return response;
}

function killGroup(child) {
  if (child.pid !== undefined) try { process.kill(-child.pid, "SIGKILL"); }
  catch (error) { if (error.code !== "ESRCH") throw error; }
}

// Mocked subprocess tests exercise these bounds only, never compilation evidence.
export async function captureProcess(program, args, options) {
  const child = spawn(program, args, { cwd: options.cwd, env: options.env, detached: true,
    stdio: ["ignore", "pipe", "pipe"] });
  let error, stdout = Buffer.alloc(0), stderr = Buffer.alloc(0);
  const fail = (reason) => { error ??= reason; killGroup(child); };
  const timer = setTimeout(() => fail(new Error("process timeout")), options.timeout);
  const guard = options.guard && setInterval(() => { try { options.guard(); } catch (reason) { fail(reason); } }, 1000);
  child.once("error", fail);
  for (const [stream, maximum] of [["stdout", options.stdoutLimit], ["stderr", options.stderrLimit]]) {
    child[stream].on("data", (chunk) => {
      const prior = stream === "stdout" ? stdout : stderr;
      if (prior.length + chunk.length > maximum) return fail(new Error(`${stream} byte bound exceeded`));
      if (stream === "stdout") stdout = Buffer.concat([prior, chunk]); else stderr = Buffer.concat([prior, chunk]);
    });
  }
  const closed = await new Promise((done) => child.once("close", (code, signal) => done({ code, signal })));
  clearTimeout(timer);
  if (guard) clearInterval(guard);
  if (error) throw error;
  assert.equal(closed.code, 0, `process exit ${closed.code}/${closed.signal}: ${stderr.toString("utf8")}`);
  return { stdout, stderr, exit_code: closed.code };
}

export async function runTwoWorkgroupSmoke({ compilerRepo, inputDirectory, outputDirectory, binDirectory }) {
  const root = realpathSync(compilerRepo), input = realpathSync(inputDirectory);
  const output = join(realpathSync(dirname(resolve(outputDirectory))), basename(outputDirectory));
  assert.ok(!within(root, output) && !within(input, output), "output must be outside compiler and retained export");
  diskReserve(dirname(output));
  // Unchanged pure helpers: no existing script, fixture or producer modifications.
  const helperPath = join(root, "scripts/resource-query-lds-v5-smoke.mjs");
  const { SOURCE_PATH, readBounded, decodeUtf8, sha256, validateExport } = await import(pathToFileURL(helperPath).href);
  const files = { source: readBounded(join(root, SOURCE_PATH), 1024 * 1024),
    bundle: readBounded(join(input, "kernel-v5.fe2sim"), 64 * 1024 * 1024),
    stdout: readBounded(join(input, "export-stdout.txt"), 8 * 1024 * 1024),
    stderr: readBounded(join(input, "export-stderr.txt"), 8 * 1024 * 1024) };
  const metadataBytes = readBounded(join(input, "export.json"), 65536);
  const metadata = validateExport(JSON.parse(decodeUtf8(metadataBytes)), files);
  const bin = realpathSync(binDirectory), simulator = join(bin, "fe2o3-kir-sim"), debuggerPath = join(bin, "fe2o3-debug");
  const digestFile = (path, bound = 64 * 1024 * 1024) => sha256(readBounded(path, bound));
  const executableHashes = { simulator: digestFile(simulator), debugger: digestFile(debuggerPath) };
  const helperHash = digestFile(helperPath, 1024 * 1024), scriptHash = digestFile(fileURLToPath(import.meta.url), 1024 * 1024);
  mkdirSync(output); // Existing output is refused; no removal/reuse.
  const env = { PATH: "/usr/local/bin:/usr/bin:/bin", LANG: "C.UTF-8", LC_ALL: "C.UTF-8" };
  const save = (name, bytes) => {
    diskReserve(output, Buffer.byteLength(bytes));
    writeFileSync(join(output, name), bytes, { flag: "wx" });
  };
  const saveJson = (name, value) => {
    const text = `${JSON.stringify(value, null, 2)}\n`;
    assert.ok(Buffer.byteLength(text) <= LIMITS.result_bytes, "result byte bound exceeded"); save(name, text); return text;
  };
  const profile = twoWorkgroupProfile(), requestBytes = `${JSON.stringify(profile.request, null, 2)}\n`;
  const bundlePath = join(output, "kernel-v5.fe2sim"), requestPath = join(output, "simulation-request.json");
  const requestLines = [], responseLines = [];
  let child, pending, failure, processTimer, diskTimer, closed, closing = false, stderr = "", buffer = "";
  let responseBytes = 0, requestTotal = 0, revision = 0, nextId = 1, configuration, activeWatchpoint;
  const fail = (error) => {
    failure ??= error; pending?.reject(error); pending = undefined;
    if (child) killGroup(child);
  };
  async function ask(fields) {
    if (failure) throw failure;
    assert.equal(pending, undefined); assert.ok(nextId <= LIMITS.commands, "command budget exhausted");
    const request = { schema: "fe2o3-debug-request-v1", request_id: nextId++, expected_revision: revision, ...fields };
    const line = `${JSON.stringify(request)}\n`; requestTotal += Buffer.byteLength(line);
    assert.ok(Buffer.byteLength(line) <= LIMITS.request_bytes && requestTotal <= LIMITS.transcript_bytes);
    requestLines.push(line);
    const response = await new Promise((accept, reject) => {
      const timer = setTimeout(() => fail(new Error("debugger response timeout")), LIMITS.reply_timeout_ms);
      pending = { resolve(value) { clearTimeout(timer); accept(value); }, reject(error) { clearTimeout(timer); reject(error); } };
      child.stdin.write(line, (error) => { if (error) fail(error); });
    });
    assert.equal(response.request_id, request.request_id); assert.equal(response.operation, request.operation);
    assert.equal(response.schema, request.schema === "fe2o3-debug-resource-request-v1" ? "fe2o3-debug-resource-response-v1" : "fe2o3-debug-response-v1");
    assert.equal(response.session.backend, "cpu_kir_simulator"); assert.equal(response.session.execution_kind, "cpu_kir_simulation");
    assert.equal(response.session.simulated, true); assert.equal(response.session.hardware_observed, false);
    assert.equal(response.session.performance_prediction, false); assert.ok(safe(response.session.revision));
    assert.equal(response.session.cursor.state_revision, response.session.revision);
    configuration ??= response.session.configuration_identity;
    assert.equal(response.session.configuration_identity, configuration); assert.equal(response.session.cursor.configuration_identity, configuration);
    revision = response.session.revision; return response;
  }
  async function ok(fields) { const response = await ask(fields); assert.equal(response.status, "ok", JSON.stringify(response.error)); return response; }
  async function rejected(fields, code) {
    const before = revision, response = await ask(fields);
    assert.equal(response.status, "error"); assert.equal(response.error.code, code);
    assert.equal(response.error.state_changed, false); assert.equal(revision, before); return response;
  }
  async function step(direction) {
    const response = await ok({ operation: "step", direction, granularity: "operation", count: 1 });
    return { anchor_response: response, anchor: validateCheckpoint(response) };
  }
  async function resource(operation, anchor, fields = {}) {
    const response = await ok({ schema: "fe2o3-debug-resource-request-v1", operation, expected_snapshot: anchor,
      page: { max_items: LIMITS.page_items, max_scanned: LIMITS.page_scanned }, ...fields });
    same(response.snapshot, anchor); assert.equal(response.physical_registers, "not_represented");
    assert.equal(response.page.completeness.status, "complete");
    assert.ok(safe(response.page.scanned) && response.page.scanned <= (fields.page?.max_scanned ?? LIMITS.page_scanned));
    assert.ok(safe(response.page.source_count) && response.page.source_count <= LIMITS.records);
    return response;
  }
  async function inventory(anchor) {
    const response = await resource("query_allocations", anchor);
    assert.equal(response.result.result, "allocations"); assert.equal(response.page.next_token, undefined);
    return { response, ...validateAllocationRows(response.result.allocations) };
  }
  async function memory(anchor, allocation, length, expectedBytes, expectedInitialization) {
    const response = await ok({ operation: "read_memory", allocation, byte_offset: 0, byte_len: length });
    same(response.result.snapshot, anchor); same(response.result.memory.allocation, allocation);
    assert.equal(response.result.memory.byte_offset, 0); assert.equal(response.result.memory.requested_bytes, length);
    const availability = response.result.memory.availability;
    if (expectedInitialization === null) {
      assert.equal(response.result.memory.returned_bytes, 0);
      same(availability, { status: "unavailable", reason: "not_represented" });
    } else {
      assert.equal(response.result.memory.returned_bytes, length); assert.equal(availability.status, "captured");
      assert.equal(availability.truncated, false); assert.equal(availability.address_space, length === 520 ? "global" : "workgroup");
      assert.match(availability.bytes, new RegExp(`^0x[0-9a-f]{${length * 2}}$`, "u"));
      if (expectedBytes !== undefined) assert.equal(availability.bytes, expectedBytes);
      assert.equal(availability.initialized, expectedInitialization);
    }
    return response;
  }
  const filter = (workgroup, addressSpace, allocation) => ({ scope: { level: "workgroup", workgroup: [workgroup, 0, 0] }, address_space: addressSpace, allocation });
  async function accesses(anchor, workgroup, addressSpace, allocation) {
    const rows = [], pageIds = [], tokens = new Set(); let token, sourceCount, scanned = 0, previous = 0;
    for (let index = 0; index < LIMITS.pages; index++) {
      const response = await resource("query_memory_accesses", anchor, { filter: filter(workgroup, addressSpace, allocation),
        page: { max_items: LIMITS.page_items, max_scanned: LIMITS.page_scanned, ...(token ? { token } : {}) } });
      assert.equal(response.result.result, "memory_accesses"); assert.ok(response.result.accesses.length <= LIMITS.page_items);
      pageIds.push(response.request_id); sourceCount ??= response.page.source_count;
      assert.equal(response.page.source_count, sourceCount); scanned += response.page.scanned;
      for (const row of response.result.accesses) { previous = validateAccessRow(row, anchor, workgroup, addressSpace, allocation, previous); rows.push(row); }
      token = response.page.next_token;
      if (token === undefined) { assert.equal(scanned, sourceCount); return { rows, page_request_ids: pageIds, raw_records_scanned: scanned }; }
      assert.equal(typeof token, "string"); assert.ok(token.length <= 256 && !tokens.has(token)); tokens.add(token);
    }
    throw new Error("access page budget exhausted");
  }
  async function watch(allocation, offset, label) {
    const response = await replaceWatchpoint(ok, activeWatchpoint, allocation, offset, label);
    activeWatchpoint = response.result.stop.watchpoint_id; return response;
  }
  const checkTreeAccesses = (observed) => {
    assert.equal(observed.rows.length, 1280);
    assert.equal(observed.rows.filter((row) => row.access === "write_committed").length, 448);
    assert.equal(observed.rows.filter((row) => row.access === "read").length, 832);
  };
  try {
    for (const [name, bytes] of [["source.rs", files.source], ["kernel-v5.fe2sim", files.bundle], ["export.json", metadataBytes],
      ["export-stdout.txt", files.stdout], ["export-stderr.txt", files.stderr], ["simulation-request.json", requestBytes]]) save(name, bytes);
    const simulation = await captureProcess(simulator, ["--bundle-v5", bundlePath, "--request", requestPath],
      { cwd: root, env, timeout: LIMITS.process_timeout_ms, stdoutLimit: LIMITS.response_bytes,
        stderrLimit: LIMITS.stderr_bytes, guard: () => diskReserve(output) });
    save("simulation-stdout.json", simulation.stdout); save("simulation-stderr.txt", simulation.stderr);
    const result = JSON.parse(decodeUtf8(simulation.stdout));
    assert.equal(result.schema, "fe2o3-simulation-result-v1"); assert.equal(result.status, "ok");
    assert.equal(result.authority, "observation_only"); assert.equal(result.simulated, true); assert.equal(result.hardware_observed, false);
    assert.equal(result.hardware_validation, false); assert.equal(result.performance_prediction, false);
    assert.equal(result.counts.invocations_executed, 128); assert.equal(result.counts.workgroups_visited, 2);
    assert.equal(result.schedule.coverage.complete, true); assert.equal(result.schedule.coverage.workgroups, 2);
    assert.equal(result.schedule.identity, "workgroup_major_local_zyx_cooperative_v1");
    assert.equal(result.arguments[0].bits, "0x00000002"); assert.equal(result.arguments[1].value.bytes, profile.final);
    assert.equal(result.arguments[1].value.initialized, profile.global_initialized);
    child = spawn(debuggerPath, ["sim", "--bundle-v5", bundlePath, "--request", requestPath, "--wave-width", "32"],
      { cwd: root, env, detached: true, stdio: ["pipe", "pipe", "pipe"] });
    processTimer = setTimeout(() => fail(new Error("debugger process timeout")), LIMITS.process_timeout_ms);
    diskTimer = setInterval(() => { try { diskReserve(output); } catch (error) { fail(error); } }, 1000);
    closed = new Promise((done) => child.once("close", (code, signal) => done({ code, signal })));
    const decoder = new TextDecoder("utf-8", { fatal: true }), stderrDecoder = new TextDecoder("utf-8", { fatal: true });
    child.once("error", fail); child.stdin.on("error", fail);
    child.stderr.on("data", (chunk) => { try {
      stderr += stderrDecoder.decode(chunk, { stream: true }); assert.ok(Buffer.byteLength(stderr) <= LIMITS.stderr_bytes);
    } catch (error) { fail(error); } });
    child.stdout.on("data", (chunk) => { try {
      responseBytes += chunk.length; buffer += decoder.decode(chunk, { stream: true });
      assert.ok(responseBytes <= LIMITS.transcript_bytes && Buffer.byteLength(buffer) <= LIMITS.response_bytes);
      const newline = buffer.indexOf("\n"); if (newline < 0) return;
      assert.ok(pending, "unsolicited debugger response");
      const line = buffer.slice(0, newline + 1), response = JSON.parse(line);
      buffer = buffer.slice(newline + 1); assert.equal(buffer, "", "multiple replies for one request");
      responseLines.push(line); const completing = pending; pending = undefined; completing.resolve(response);
    } catch (error) { fail(error); } });
    child.once("close", (code, signal) => {
      clearTimeout(processTimer);
      clearInterval(diskTimer);
      try { assert.equal(decoder.decode(), ""); assert.equal(stderrDecoder.decode(), ""); assert.equal(buffer, ""); }
      catch (error) { fail(error); }
      if (!closing || pending || code !== 0) fail(new Error(`unexpected debugger exit ${code}/${signal}: ${stderr}`));
    });
    let pre0, initialInventory;
    for (let index = 0; index < LIMITS.discovery_steps; index++) {
      pre0 = await step("forward"); same(pre0.anchor.scope.workgroup, [0, 0, 0]);
      initialInventory = await inventory(pre0.anchor); if (initialInventory.workgroup) break;
    }
    assert.ok(initialInventory.workgroup, "WG0 allocation not captured within discovery budget");
    const global = clone(initialInventory.global), lds0 = clone(initialInventory.workgroup);
    const pre0Memory = await memory(pre0.anchor, lds0, 256, undefined, profile.uninitialized_scratch);
    const pre0Global = await memory(pre0.anchor, global, 520, profile.initial, profile.global_initialized);
    const pre0Accesses = await accesses(pre0.anchor, 0, "workgroup", lds0); assert.equal(pre0Accesses.rows.length, 0);
    const stop0 = await watch(global, 0, "wg0-first-global-write");
    const reduction0 = await step("forward"), inventory0 = await inventory(reduction0.anchor);
    same(reduction0.anchor.scope.workgroup, [0, 0, 0]); same(inventory0.global, global); same(inventory0.workgroup, lds0);
    const reduction0Memory = await memory(reduction0.anchor, lds0, 256, profile.scratch, profile.scratch_initialized);
    const reduction0Global = await memory(reduction0.anchor, global, 520, profile.afterWords(1), profile.global_initialized);
    const accesses0 = await accesses(reduction0.anchor, 0, "workgroup", lds0); checkTreeAccesses(accesses0);
    const stopLast0 = await watch(global, 252, "wg0-last-global-write");
    const last0 = await step("forward"), last0Inventory = await inventory(last0.anchor);
    same(last0.anchor.scope.workgroup, [0, 0, 0]); same(last0Inventory.workgroup, lds0);
    const last0Memory = await memory(last0.anchor, lds0, 256, profile.scratch, profile.scratch_initialized);
    const last0Global = await memory(last0.anchor, global, 520, profile.afterWords(64), profile.global_initialized);
    const transitionPath = [last0]; let empty, pre1, inventory1;
    for (let index = 0; index < LIMITS.discovery_steps; index++) {
      const current = await step("forward"), observed = await inventory(current.anchor); transitionPath.push(current);
      same(observed.global, global);
      if (current.anchor.scope.workgroup[0] === 0) { same(observed.workgroup, lds0); continue; }
      assert.ok(!observed.response.result.allocations.some((row) => row.allocation.ordinal === lds0.ordinal));
      if (observed.workgroup) { pre1 = current; inventory1 = observed; break; }
      empty ??= { ...current, inventory: observed.response,
        prior_window: await memory(current.anchor, lds0, 256, undefined, null) };
    }
    assert.ok(empty, "no exact global-only WG1 checkpoint observed; do not infer a release event");
    assert.ok(pre1, "WG1 allocation not captured within discovery budget");
    const lds1 = clone(inventory1.workgroup); assert.notEqual(lds1.ordinal, lds0.ordinal);
    const pre1Memory = await memory(pre1.anchor, lds1, 256, undefined, profile.uninitialized_scratch);
    const oldUnavailable = await memory(pre1.anchor, lds0, 256, undefined, null);
    const pre1Global = await memory(pre1.anchor, global, 520, profile.afterWords(64), profile.global_initialized);
    const priorTokenPage = await resource("query_memory_accesses", pre1.anchor,
      { filter: filter(1, "workgroup", lds1), page: { max_items: 1, max_scanned: 1 } });
    assert.equal(typeof priorTokenPage.page.next_token, "string");
    // Reverse the exact observed path, not an inferred cursor arithmetic jump.
    let reversed;
    for (let index = transitionPath.length - 2; index >= 0; index--) {
      reversed = await step("reverse"); sameCheckpointLocation(reversed.anchor, transitionPath[index].anchor);
    }
    assert.notEqual(reversed.anchor.cursor.state_revision, last0.anchor.cursor.state_revision);
    const restored0Inventory = await inventory(reversed.anchor); same(restored0Inventory.workgroup, lds0);
    const restored0Memory = await memory(reversed.anchor, lds0, 256, profile.scratch, profile.scratch_initialized);
    const futureUnavailable = await memory(reversed.anchor, lds1, 256, undefined, null);
    const noFuture = await accesses(reversed.anchor, 1, "workgroup", lds1); assert.equal(noFuture.rows.length, 0);
    let restored1;
    for (let index = 1; index < transitionPath.length; index++) {
      restored1 = await step("forward"); sameCheckpointLocation(restored1.anchor, transitionPath[index].anchor);
    }
    assert.notEqual(restored1.anchor.cursor.state_revision, pre1.anchor.cursor.state_revision);
    const restored1Inventory = await inventory(restored1.anchor); same(restored1Inventory.workgroup, lds1);
    const restored1Memory = await memory(restored1.anchor, lds1, 256, pre1Memory.result.memory.availability.bytes, profile.uninitialized_scratch);
    const historical0 = await accesses(restored1.anchor, 0, "workgroup", lds0); checkTreeAccesses(historical0);
    const empty1 = await accesses(restored1.anchor, 1, "workgroup", lds1); assert.equal(empty1.rows.length, 0);
    const cross0 = await accesses(restored1.anchor, 0, "workgroup", lds1); assert.equal(cross0.rows.length, 0);
    const cross1 = await accesses(restored1.anchor, 1, "workgroup", lds0); assert.equal(cross1.rows.length, 0);
    const staleAnchor = await rejected({ schema: "fe2o3-debug-resource-request-v1", operation: "query_allocations",
      expected_revision: pre1.anchor.cursor.state_revision, expected_snapshot: pre1.anchor,
      page: { max_items: 1, max_scanned: 1 } }, "stale_revision");
    const staleToken = await rejected({ schema: "fe2o3-debug-resource-request-v1", operation: "query_memory_accesses",
      expected_snapshot: restored1.anchor, filter: filter(1, "workgroup", lds1),
      page: { max_items: 1, max_scanned: 1, token: priorTokenPage.page.next_token } }, "invalid_cursor");
    const tokenPage = await resource("query_memory_accesses", restored1.anchor,
      { filter: filter(0, "workgroup", lds0), page: { max_items: 1, max_scanned: 1 } });
    assert.equal(typeof tokenPage.page.next_token, "string");
    const crossedToken = await rejected({ schema: "fe2o3-debug-resource-request-v1", operation: "query_memory_accesses",
      expected_snapshot: restored1.anchor, filter: filter(1, "workgroup", lds1),
      page: { max_items: 1, max_scanned: 1, token: tokenPage.page.next_token } }, "invalid_cursor");
    const stop1 = await watch(global, 256, "wg1-first-global-write");
    const reduction1 = await step("forward"), reduction1Inventory = await inventory(reduction1.anchor);
    same(reduction1.anchor.scope.workgroup, [1, 0, 0]); same(reduction1Inventory.global, global); same(reduction1Inventory.workgroup, lds1);
    const reduction1Memory = await memory(reduction1.anchor, lds1, 256, profile.scratch, profile.scratch_initialized);
    const reduction1Global = await memory(reduction1.anchor, global, 520, profile.afterWords(65), profile.global_initialized);
    const accesses1 = await accesses(reduction1.anchor, 1, "workgroup", lds1); checkTreeAccesses(accesses1);
    const firstGlobal1 = await accesses(reduction1.anchor, 1, "global", global);
    assert.equal(firstGlobal1.rows.length, 1); assert.equal(firstGlobal1.rows[0].range.byte_offset, "256");
    await ok({ operation: "remove_watchpoints", watchpoint_ids: [activeWatchpoint] });
    const completed = await ok({ operation: "continue", max_events: LIMITS.records });
    assert.equal(completed.result.stop.reason, "completed"); assert.equal(completed.result.stop.outcome, "completed"); assert.equal(completed.result.stop.exact, true);
    const terminalMemory = await ask({ operation: "read_memory", allocation: lds1, byte_offset: 0, byte_len: 256 });
    assert.equal(terminalMemory.status, "unavailable");
    const final = await step("reverse"), finalInventory = await inventory(final.anchor);
    same(final.anchor.scope.workgroup, [1, 0, 0]); same(finalInventory.workgroup, lds1);
    const finalGlobal = await memory(final.anchor, global, 520, profile.final, profile.global_initialized);
    const finalAccesses = [];
    for (const workgroup of [0, 1]) {
      const observed = await accesses(final.anchor, workgroup, "global", global);
      same(observed.rows.map((row) => row.range.byte_offset), Array.from({ length: 64 }, (_, index) => String((workgroup * 64 + index) * 4)));
      assert.ok(observed.rows.every((row) => row.access === "write_committed")); finalAccesses.push(observed);
    }
    closing = true; await ok({ operation: "terminate" }); child.stdin.end(); same(await closed, { code: 0, signal: null });
    if (failure) throw failure;
    assert.equal(digestFile(join(root, SOURCE_PATH), 1024 * 1024), metadata.source_sha256);
    assert.equal(digestFile(join(input, "kernel-v5.fe2sim")), metadata.bundle_sha256);
    same({ simulator: digestFile(simulator), debugger: digestFile(debuggerPath) }, executableHashes);
    assert.equal(digestFile(helperPath, 1024 * 1024), helperHash); assert.equal(digestFile(fileURLToPath(import.meta.url), 1024 * 1024), scriptHash);
    const requestText = requestLines.join(""), responseText = responseLines.join("");
    save("debug-requests.jsonl", requestText); save("debug-responses.jsonl", responseText); save("debug-stderr.txt", stderr);
    const resultsText = saveJson("resource-query-results.json", {
      schema: "fe2o3-lds-two-workgroups-query-checkpoints-v1",
      pre_workgroup0: { ...pre0, inventory: initialInventory.response, memory: pre0Memory, global: pre0Global, accesses: pre0Accesses },
      reduction_workgroup0: { ...reduction0, stop: stop0, inventory: inventory0.response, memory: reduction0Memory, global: reduction0Global, accesses: accesses0 },
      last_write_workgroup0: { ...last0, stop: stopLast0, inventory: last0Inventory.response, memory: last0Memory, global: last0Global },
      transition_path: transitionPath, global_only_workgroup1: empty,
      pre_workgroup1: { ...pre1, inventory: inventory1.response, memory: pre1Memory, prior_window: oldUnavailable, global: pre1Global },
      reverse_workgroup0: { ...reversed, inventory: restored0Inventory.response, memory: restored0Memory, future_window: futureUnavailable, future_accesses: noFuture },
      forward_workgroup1: { ...restored1, inventory: restored1Inventory.response, memory: restored1Memory, historical_workgroup0_accesses: historical0,
        current_accesses: empty1, cross_scope_accesses: [cross0, cross1] },
      negatives: { stale_anchor: staleAnchor, prior_token_page: priorTokenPage, stale_token: staleToken,
        token_page: tokenPage, crossed_token: crossedToken },
      reduction_workgroup1: { ...reduction1, stop: stop1, inventory: reduction1Inventory.response, memory: reduction1Memory, global: reduction1Global, accesses: accesses1, global_accesses: firstGlobal1 },
      completion_response: completed, terminal_memory_response: terminalMemory,
      final: { ...final, inventory: finalInventory.response, global: finalGlobal, global_accesses: finalAccesses,
        interpretation: "last captured operation; not a post-release or allocation-lifetime checkpoint" },
    });
    saveJson("receipt.json", { schema: "fe2o3-lds-two-workgroups-source-smoke-v1",
      source_path: SOURCE_PATH, source_sha256: metadata.source_sha256, bundle_sha256: metadata.bundle_sha256,
      export_observation_sha256: sha256(metadataBytes), simulation_request_sha256: sha256(requestBytes),
      simulation_stdout_sha256: sha256(simulation.stdout), debug_requests_sha256: sha256(requestText),
      debug_responses_sha256: sha256(responseText), results_sha256: sha256(resultsText), script_sha256: scriptHash,
      helper_sha256: helperHash, executable_sha256: executableHashes, commands: requestLines.length, response_bytes: responseBytes,
      grid: [128, 1, 1], workgroup: [64, 1, 1], logical_wave_width: 32, expected_u32: 128,
      observed_workgroup_allocations: [lds0, lds1], global_allocation: global, transition_steps: transitionPath.length - 1,
      evidence_kind: "actual_v5_admission_and_cpu_simulation_observation", source_edited: false,
      metadata_authority: "inert_cross_file_consistency_not_compiler_authentication", compiler_closure_attestation: "unavailable",
      allocation_generation: "producer_profile_zero_not_lifetime_evidence", owning_scope: "not_represented",
      lifetime: "not_represented", physical_base: "not_represented", physical_registers: "not_represented",
      source_helper_or_loop_qualification: false, allocation_release_event_captured: false, physical_reuse_observed: false,
      hardware_observed: false, performance_prediction: false, grants_production_resume: false,
      grants_load_authority: false, grants_launch_authority: false, script_limits: LIMITS,
      disk_reserve_bytes: String(DISK_RESERVE),
      cli_capture_limits: { records: 1000000, retained_memory_bytes: 268435456,
        interpretation: "existing CLI hard limits; script selection bounds do not configure runtime capture" },
      checks: ["same_actual_source_and_v5_bundle", "128_output_words_and_canaries", "two_distinct_observed_lds_ids",
        "exact_current_inventory_and_windows", "global_only_transition_checkpoint", "reverse_forward_restoration",
        "scope_filtered_actual_access_joins", "historical_accesses_not_current_liveness", "no_future_accesses",
        "same_cursor_new_revision", "stale_anchor_and_cross_filter_token_refused", "terminal_snapshot_unavailable", "source_unchanged"] });
    console.log(`Actual-source V5 two-workgroup LDS observation passed; evidence: ${output}`);
  } catch (error) {
    fail(error); if (closed) await closed;
    saveJson("failure.json", { status: "failed", message: String(error), manufactured_capture_fallback: false, hardware_observed: false });
    save("partial-debug-requests.jsonl", requestLines.join("")); save("partial-debug-responses.jsonl", responseLines.join(""));
    save("failure-stderr.txt", stderr); throw error;
  } finally { clearTimeout(processTimer); clearInterval(diskTimer); }
}

export function parseArgs(args) {
  const allowed = new Set(["--compiler-repo", "--export-directory", "--output", "--bin-directory"]), values = {};
  assert.equal(args.length, 8, "usage: --compiler-repo DIR --export-directory DIR --output NEW_DIR --bin-directory DIR");
  for (let index = 0; index < args.length; index += 2) {
    assert.ok(allowed.has(args[index]) && values[args[index]] === undefined);
    assert.ok(typeof args[index + 1] === "string" && args[index + 1].length <= 4096 && !args[index + 1].includes("\0"));
    assert.ok(isAbsolute(args[index + 1])); values[args[index]] = args[index + 1];
  }
  return { compilerRepo: values["--compiler-repo"], inputDirectory: values["--export-directory"],
    outputDirectory: values["--output"], binDirectory: values["--bin-directory"] };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await runTwoWorkgroupSmoke(parseArgs(process.argv.slice(2)));
