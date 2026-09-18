import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { LIMITS, captureProcess, parseArgs, replaceWatchpoint, sameCheckpointLocation,
  twoWorkgroupProfile, validateAccessRow, validateAllocationRows, validateCheckpoint,
} from "./resource-query-lds-multi-workgroup-v5-smoke.mjs";

// Synthetic DTOs/processes below are negative/contract controls, never admitted
// bundles, source-produced captures, ownership facts or successful smoke receipts.
const clone = (value) => JSON.parse(JSON.stringify(value));
function rows() {
  const row = (ordinal, space, capacity) => ({ allocation: { ordinal, generation: 0 }, address_space: space,
    capacity_bytes: capacity, alignment: 4, access: "read_write", snapshot_bytes_available: true,
    initialization_available: true, owning_scope: "not_represented", lifetime: "not_represented", physical_base: "not_represented" });
  return [row(1, "global", "520"), row(2, "workgroup", "256")];
}
function checkpoint() {
  const cursor = { configuration_identity: "a".repeat(64), event_sequence: 10, state_revision: 3 };
  return { status: "ok", session: { cursor: clone(cursor) }, result: { stop: { exact: true }, snapshot: { status: "captured", snapshot: {
    anchor: { cursor, scope: { level: "lane", interpretation: "logical_visualization", wave_width: 32,
      active_mask: 4294967295, workgroup: [0, 0, 0], wave: 0, lane: 0, logical_workitem: [0, 0, 0] },
    site: { kir: { function_ordinal: 0, block_ordinal: 0, point: { kind: "operation", operation_ordinal: 0 } }, source: { status: "unavailable" } } },
  } } } };
}
function access() {
  return { allocation: { ordinal: 2, generation: 0 }, address_space: "workgroup", access: "write_committed",
    range: { byte_offset: "0", byte_len: "4" }, call_frame: "not_represented", operation_occurrence: "not_represented",
    source_association: "not_represented", occurrence: { record_ordinal: 7, event_sequence: 8,
      scope: { workgroup: [0, 0, 0], wave_width: 32, active_mask: 4294967295, interpretation: "logical_visualization" },
      schedule: { identity: "workgroup_major_local_zyx_cooperative_v1" } } };
}

test("same source profile has two 64-lane groups and exact 128-word/canary oracle", () => {
  const profile = twoWorkgroupProfile();
  assert.deepEqual(profile.request.grid, [128, 1, 1]); assert.deepEqual(profile.request.workgroup, [64, 1, 1]);
  assert.equal(profile.request.arguments[0].bits, "0x00000002");
  assert.equal(profile.initial, `0x${"a5".repeat(512)}deadbeefcafebabe`);
  assert.equal(profile.afterWords(64), `0x${"80000000".repeat(64)}${"a5".repeat(256)}deadbeefcafebabe`);
  assert.equal(profile.afterWords(65), `0x${"80000000".repeat(65)}${"a5".repeat(252)}deadbeefcafebabe`);
  assert.equal(profile.final, `0x${"80000000".repeat(128)}deadbeefcafebabe`);
  assert.equal(profile.global_initialized, `0x${"ff".repeat(65)}`);
  assert.deepEqual(profile.scratch_words, [128, 64, 32, 32, ...Array(4).fill(16), ...Array(8).fill(8), ...Array(16).fill(4), ...Array(32).fill(2)]);
  for (const bad of [-1, 129, NaN, 1.5, "1"]) assert.throws(() => profile.afterWords(bad));
  profile.request.grid[0] = 64; assert.equal(twoWorkgroupProfile().request.grid[0], 128);
});

test("allocation inventory preserves unavailable facts and refuses generations, aliases and wrong capacities", () => {
  assert.deepEqual(validateAllocationRows(rows()), { global: { ordinal: 1, generation: 0 }, workgroup: { ordinal: 2, generation: 0 } });
  assert.equal(validateAllocationRows(rows().slice(0, 1)).workgroup, undefined);
  const mutations = [
    (value) => { value[1].allocation.ordinal = 1; }, (value) => { value[1].allocation.generation = 1; },
    (value) => { value[1].allocation.ordinal = Number.MAX_SAFE_INTEGER + 1; }, (value) => { value[1].capacity_bytes = "0256"; },
    (value) => { value[0].capacity_bytes = "264"; }, (value) => { value[1].address_space = "private"; },
    (value) => { value[1].owning_scope = { workgroup: [0, 0, 0] }; }, (value) => { value[1].lifetime = "released"; },
    (value) => { value[1].physical_base = 0; }, (value) => { value[1].initialization_available = false; },
    (value) => { value.push(clone(value[1])); }, (value) => { value.splice(0, 1); },
  ];
  for (const change of mutations) { const value = rows(); change(value); assert.throws(() => validateAllocationRows(value)); }
});

test("checkpoint copy and reverse identity match every anchor field except actual new revision", () => {
  const original = checkpoint(), anchor = validateCheckpoint(original), restored = clone(anchor);
  restored.cursor.state_revision = 9; assert.doesNotThrow(() => sameCheckpointLocation(restored, anchor));
  original.result.snapshot.snapshot.anchor.scope.workgroup[0] = 1;
  assert.equal(anchor.scope.workgroup[0], 0, "caller mutation must not change retained anchor");
  const changes = [
    (value) => { value.cursor.state_revision = anchor.cursor.state_revision; },
    (value) => { value.cursor.configuration_identity = "b".repeat(64); },
    (value) => { value.cursor.event_sequence++; }, (value) => { value.scope.workgroup[0] = 1; },
    (value) => { value.scope.lane = 1; }, (value) => { value.site.kir.point.operation_ordinal = 1; },
  ];
  for (const change of changes) { const value = clone(restored); change(value); assert.throws(() => sameCheckpointLocation(value, anchor)); }
  for (const change of [
    (value) => { value.result.stop.exact = false; }, (value) => { value.session.cursor.event_sequence++; },
    (value) => { value.result.snapshot.snapshot.anchor.scope.wave_width = 64; },
    (value) => { value.result.snapshot.snapshot.anchor.scope.active_mask = Number.MAX_SAFE_INTEGER + 1; },
    (value) => { value.result.snapshot.snapshot.anchor.scope.workgroup[0] = 2; },
  ]) { const value = checkpoint(); change(value); assert.throws(() => validateCheckpoint(value)); }
});

test("access joins reject future, cross-group, wrong allocation, duplicate sequence and lossy ranges", () => {
  const anchor = validateCheckpoint(checkpoint()), allocation = { ordinal: 2, generation: 0 };
  assert.equal(validateAccessRow(access(), anchor, 0, "workgroup", allocation, 0), 8);
  const changes = [
    (value) => { value.occurrence.event_sequence = 11; value.occurrence.record_ordinal = 10; },
    (value) => { value.occurrence.record_ordinal = 6; }, (value) => { value.occurrence.scope.workgroup[0] = 1; },
    (value) => { value.allocation.ordinal = 3; }, (value) => { value.allocation.generation = 1; },
    (value) => { value.range.byte_offset = "00"; }, (value) => { value.range.byte_offset = "256"; },
    (value) => { value.range.byte_offset = "9007199254740993"; }, (value) => { value.range.byte_len = "8"; },
    (value) => { value.source_association = "compiler_bound"; }, (value) => { value.call_frame = 0; },
    (value) => { value.occurrence.scope.wave_width = 64; }, (value) => { value.access = "read_modify_write"; },
  ];
  for (const change of changes) { const value = access(); change(value); assert.throws(() => validateAccessRow(value, anchor, 0, "workgroup", allocation, 0)); }
  assert.throws(() => validateAccessRow(access(), anchor, 0, "workgroup", allocation, 8));
  const global = access(); global.address_space = "global"; global.occurrence.scope.workgroup = [1, 0, 0]; global.range.byte_offset = "256";
  assert.equal(validateAccessRow(global, anchor, 1, "global", allocation, 0), 8);
  for (const offset of ["0", "252", "512", "516"]) { global.range.byte_offset = offset; assert.throws(() => validateAccessRow(global, anchor, 1, "global", allocation, 0)); }
});

test("watchpoint replacement binds exact nonzero offsets and observed ID, never an assumed ID", async () => {
  const commands = [], allocation = { ordinal: 1, generation: 0 }; let spec;
  const ask = async (command) => {
    commands.push(command);
    if (command.operation === "set_watchpoints") [spec] = command.watchpoints;
    if (command.operation === "list_watchpoints") return { result: { watchpoints: [{ spec, watchpoint_id: 17 }] } };
    return { result: { stop: { reason: "watchpoint", exact: true, watchpoint_id: 17 } } };
  };
  await replaceWatchpoint(ask, 4, allocation, 252, "last-wg0-write");
  assert.deepEqual(commands.map((command) => command.operation), ["remove_watchpoints", "set_watchpoints", "list_watchpoints", "continue"]);
  assert.equal(commands[1].watchpoints[0].byte_offset, 252);
  for (const bad of [-1, 1, 512, NaN, "256"]) await assert.rejects(replaceWatchpoint(ask, undefined, allocation, bad, "control"));
  for (const mutation of ["wrong_id", "wrong_offset", "wrong_stop", "extra"]) {
    const changed = async (command) => {
      const response = await ask(command);
      if (command.operation === "continue" && mutation === "wrong_id") response.result.stop.watchpoint_id = 18;
      if (command.operation === "continue" && mutation === "wrong_stop") response.result.stop.reason = "completed";
      if (command.operation === "list_watchpoints" && mutation === "wrong_offset") response.result.watchpoints[0].spec = { ...spec, byte_offset: 0 };
      if (command.operation === "list_watchpoints" && mutation === "extra") response.result.watchpoints.push({ spec, watchpoint_id: 18 });
      return response;
    };
    await assert.rejects(replaceWatchpoint(changed, undefined, allocation, 256, "first-wg1-write"));
  }
});

test("child capture waits for close and refuses nonzero, oversized, timeout and spawn failures", async () => {
  const options = { cwd: process.cwd(), env: { PATH: "/usr/bin:/bin" }, timeout: 2000, stdoutLimit: 32, stderrLimit: 32 };
  const run = (code, extra = {}) => captureProcess(process.execPath, ["-e", code], { ...options, ...extra });
  const success = await run('process.stdout.write("control");process.stderr.write("diagnostic")');
  assert.equal(success.stdout.toString(), "control"); assert.equal(success.stderr.toString(), "diagnostic");
  await assert.rejects(run('process.stdout.write("partial");process.exit(3)'), /process exit 3/u);
  await assert.rejects(run('process.stdout.write("x".repeat(33))'), /stdout byte bound/u);
  await assert.rejects(run('process.stderr.write("x".repeat(33))'), /stderr byte bound/u);
  await assert.rejects(run('setInterval(()=>{},1000)', { timeout: 80 }), /process timeout/u);
  await assert.rejects(captureProcess("/missing/fe2o3-control", [], options), /ENOENT/u);
  // An exited parent with a descendant holding pipes remains bounded as a group.
  await assert.rejects(run('const{spawn}=require("node:child_process");spawn(process.execPath,["-e","setInterval(()=>{},1000)"],{stdio:["ignore",1,2]});process.exit(0)',
    { timeout: 150 }), /process timeout/u);
});

test("script bounds are explicit and CLI only admits four distinct absolute path options", () => {
  assert.equal(LIMITS.records, LIMITS.pages * LIMITS.page_scanned);
  assert.equal(LIMITS.page_items, 256); assert.equal(LIMITS.commands, 4096); assert.equal(LIMITS.process_timeout_ms, 120000);
  const args = ["--compiler-repo", "/repo", "--export-directory", "/input", "--output", "/output", "--bin-directory", "/bin"];
  assert.deepEqual(parseArgs(args), { compilerRepo: "/repo", inputDirectory: "/input", outputDirectory: "/output", binDirectory: "/bin" });
  for (const bad of [[], args.slice(2), [...args, "--extra", "/x"], [...args.slice(0, 6), "--output", "/other"],
    args.map((value) => value === "/input" ? "relative" : value), args.map((value) => value === "/bin" ? "/nul\0path" : value)]) assert.throws(() => parseArgs(bad));
  const script = join(dirname(fileURLToPath(import.meta.url)), "resource-query-lds-multi-workgroup-v5-smoke.mjs");
  const invalid = spawnSync(process.execPath, [script], { encoding: "utf8", timeout: 5000, maxBuffer: 16384 });
  assert.equal(invalid.error, undefined); assert.notEqual(invalid.status, 0); assert.match(invalid.stderr, /usage:/u);
});
