import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { LIMITS, SOURCE_PATH, decodeUtf8, readBounded, reductionProfile, replaceWatchpoint, sha256, validateExport, waitForProcessClose } from "./resource-query-lds-v5-smoke.mjs";

// Synthetic bytes below exercise controls only. They are never admitted bundles,
// captured evidence, compiler provenance or a successful end-to-end smoke.
function controls() {
  const files = { source: Buffer.from("control source"), bundle: Buffer.from("not a bundle"),
    stdout: Buffer.from("control stdout"), stderr: Buffer.from("control stderr") };
  const metadata = { schema: "fe2o3-workgroup-u32-v5-export-observation-v1", source_path: SOURCE_PATH,
    source_sha256: sha256(files.source), bundle_sha256: sha256(files.bundle),
    command: { program: "/control/fe2o3-export-sim", args: [
      "--crate", "fe2o3_production_ranked_bounds_fixture", "--bundle-version", "5", "--target", "gfx942",
      "--output", "/control/kernel-v5.fe2sim", "--", "--manifest-path",
      "crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/Cargo.toml",
      "--package", "fe2o3-production-ranked-bounds-fixture", "--features", "workgroup_reduce_u32", "--lib",
    ], exit_code: 0, stdout_sha256: sha256(files.stdout), stderr_sha256: sha256(files.stderr) },
    target: "gfx942:xnack-", nightly: "nightly-2026-04-03", checkout_head: "a".repeat(40),
    checkout_dirty: true, compiler_closure_attestation: "unavailable" };
  return { files, metadata };
}

test("export observations bind exact files without asserting compiler authentication", () => {
  const { files, metadata } = controls();
  assert.equal(validateExport(metadata, files), metadata);
  const manifestOnlyArgs = metadata.command.args.filter((value) => !["--package", "fe2o3-production-ranked-bounds-fixture"].includes(value));
  assert.doesNotThrow(() => validateExport({ ...metadata, command: { ...metadata.command, args: manifestOnlyArgs } }, files));
  for (const field of ["source", "bundle", "stdout", "stderr"]) {
    assert.throws(() => validateExport(metadata, { ...files, [field]: Buffer.from("mutation") }), /digest mismatch/u);
  }
  for (const [field, value] of [["source_path", "../source.rs"], ["target", "gfx950:xnack-"],
    ["nightly", "stable"], ["checkout_head", "unknown"], ["checkout_dirty", "false"],
    ["compiler_closure_attestation", "authenticated"], ["bundle_sha256", "A".repeat(64)]]) {
    assert.throws(() => validateExport({ ...metadata, [field]: value }, files));
  }
  assert.throws(() => validateExport({ ...metadata, grants_compiler_authority: true }, files));
  assert.throws(() => validateExport({ ...metadata, command: { ...metadata.command, exit_code: 1 } }, files));
});

test("command metadata is bounded and selects exactly the existing V5 source profile", () => {
  const { files, metadata } = controls();
  for (const [flag, value] of [["--features", "workgroup_reduce_i32"], ["--bundle-version", "6"],
    ["--target", "gfx950"], ["--manifest-path", "/elsewhere/Cargo.toml"], ["--crate", "other"],
    ["--package", "other"], ["--output", "other.fe2sim"]]) {
    const args = [...metadata.command.args]; args[args.indexOf(flag) + 1] = value;
    assert.throws(() => validateExport({ ...metadata, command: { ...metadata.command, args } }, files));
  }
  for (const args of [[], Array(65).fill("x"), [...metadata.command.args, "--features", "workgroup_reduce_u32"],
    [...metadata.command.args, "x".repeat(4097)], [...metadata.command.args, "nul\0byte"]]) {
    assert.throws(() => validateExport({ ...metadata, command: { ...metadata.command, args } }, files));
  }
  assert.throws(() => validateExport({ ...metadata, command: { ...metadata.command, program: "sh" } }, files));
});

test("independent 64-lane reduction tree, bytes, initialization and canaries are exact", () => {
  const profile = reductionProfile();
  assert.deepEqual(profile.request.grid, [64, 1, 1]);
  assert.deepEqual(profile.request.workgroup, [64, 1, 1]);
  assert.equal(profile.request.arguments[0].bits, "0x00000002");
  assert.deepEqual(profile.scratch_words, [128, 64, 32, 32, ...Array(4).fill(16),
    ...Array(8).fill(8), ...Array(16).fill(4), ...Array(32).fill(2)]);
  assert.equal(profile.scratch.length, 2 + 256 * 2);
  assert.equal(profile.initial, `0x${"a5".repeat(256)}deadbeefcafebabe`);
  assert.equal(profile.first, `0x80000000${"a5".repeat(252)}deadbeefcafebabe`);
  assert.equal(profile.final, `0x${"80000000".repeat(64)}deadbeefcafebabe`);
  assert.equal(profile.global_initialized, `0x${"ff".repeat(33)}`);
  assert.equal(profile.scratch_initialized, `0x${"ff".repeat(32)}`);
  profile.request.arguments[0].bits = "0xffffffff";
  assert.equal(reductionProfile().request.arguments[0].bits, "0x00000002");
});

test("regular-file reads reject oversize before bounded allocation", () => {
  const directory = mkdtempSync(join(tmpdir(), "fe2o3-lds-script-controls-"));
  try {
    const path = join(directory, "bytes"); writeFileSync(path, Buffer.from([0, 1, 2, 255]));
    assert.deepEqual(readBounded(path, 4), Buffer.from([0, 1, 2, 255]));
    assert.throws(() => readBounded(path, 3), /bound exceeded/u);
    assert.throws(() => readBounded(directory, 4), /regular file/u);
    const link = join(directory, "symlink"); symlinkSync(path, link);
    assert.throws(() => readBounded(link, 4), /ELOOP/u);
    const fifo = join(directory, "fifo");
    const made = spawnSync("mkfifo", [fifo], { encoding: "utf8", timeout: 5000 });
    assert.equal(made.error, undefined); assert.equal(made.status, 0, made.stderr);
    const moduleUrl = new URL("./resource-query-lds-v5-smoke.mjs", import.meta.url).href;
    const program = `import {readBounded} from ${JSON.stringify(moduleUrl)}; try {readBounded(process.argv[1], 4); process.exit(2)} catch(error) {if (!String(error).includes("regular file")) throw error}`;
    const checked = spawnSync(process.execPath, ["--input-type=module", "-e", program, fifo],
      { encoding: "utf8", timeout: 1000, maxBuffer: 4096 });
    assert.equal(checked.error, undefined, "FIFO rejection must not block waiting for a writer");
    assert.equal(checked.status, 0, checked.stderr);
    for (const bound of [0, -1, 1.5, NaN, Infinity, LIMITS.bundle_bytes + 1]) assert.throws(() => readBounded(path, bound));
  } finally { rmSync(directory, { recursive: true }); }
});

test("metadata decoding is fatal UTF-8 and hashes retain raw bytes", () => {
  assert.equal(decodeUtf8(Buffer.from("{\"ok\":true}")), "{\"ok\":true}");
  for (const bytes of [[0xff], [0xc3, 0x28], [0xe2, 0x82]]) assert.throws(() => decodeUtf8(Buffer.from(bytes)));
  assert.notEqual(sha256(Buffer.from([0xff])), sha256(Buffer.from("\ufffd")));
});

test("process completion waits for inherited pipes and split UTF-8 after parent exit", async () => {
  // Control subprocesses only, not a simulator/capture. The grandchild waits
  // on fd3 until this test observes the parent exit, then finishes both pipes.
  const descendant = `const fs=require("node:fs");const input=fs.createReadStream(null,{fd:3});input.once("data",()=>{process.stdout.write(Buffer.from([0x82,0xac]));process.stderr.write("late stderr\\n");input.destroy();});`;
  const parent = `const{spawn}=require("node:child_process");process.stdout.write(Buffer.from([0xe2]));spawn(process.execPath,["-e",process.argv[1]],{stdio:["ignore",1,2,3]});process.exit(0);`;
  const child = spawn(process.execPath, ["-e", parent, descendant], { detached: true, stdio: ["ignore", "pipe", "pipe", "pipe"] });
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let stdout = "", stderr = "", drained = false, exitObserved = false, timer;
  const killGroup = () => {
    if (child.pid !== undefined) try { process.kill(-child.pid, "SIGKILL"); }
    catch (error) { if (error.code !== "ESRCH") throw error; }
  };
  const closed = waitForProcessClose(child).then((value) => { drained = true; return value; });
  const failures = new Promise((_, reject) => {
    timer = setTimeout(() => { killGroup(); reject(new Error("process-drain control timed out")); }, 5000);
    child.once("error", reject);
    child.stdout.on("data", (bytes) => {
      try { stdout += decoder.decode(bytes, { stream: true }); assert.ok(Buffer.byteLength(stdout) <= 16); }
      catch (error) { reject(error); }
    });
    child.stderr.on("data", (bytes) => {
      try { stderr += decodeUtf8(bytes); assert.ok(Buffer.byteLength(stderr) <= 64); }
      catch (error) { reject(error); }
    });
    child.once("exit", () => {
      try {
        assert.equal(drained, false, "exit must not complete the capture while inherited pipes remain open");
        exitObserved = true;
        child.stdio[3].end("drain now");
      } catch (error) { reject(error); }
    });
  });
  try {
    assert.deepEqual(await Promise.race([closed, failures]), { code: 0, signal: null });
    assert.equal(exitObserved, true); assert.equal(drained, true);
    stdout += decoder.decode();
    assert.equal(stdout, "€"); assert.equal(stderr, "late stderr\n");
  } finally { clearTimeout(timer); killGroup(); }
});

test("watchpoint replacement removes prior ID and binds the exact listed spec and stop", async () => {
  const commands = [], allocation = { ordinal: 2, generation: 0 };
  let spec;
  const ask = async (command) => {
    commands.push(command);
    if (command.operation === "set_watchpoints") [spec] = command.watchpoints;
    if (command.operation === "list_watchpoints") return { result: { watchpoints: [{ spec, watchpoint_id: 9 }] } };
    return { result: { stop: { reason: "watchpoint", exact: true, watchpoint_id: 9 } } };
  };
  await replaceWatchpoint(ask, 3, allocation, "control");
  assert.deepEqual(commands.map((command) => command.operation), ["remove_watchpoints", "set_watchpoints", "list_watchpoints", "continue"]);
  assert.deepEqual(commands[0].watchpoint_ids, [3]);
  assert.deepEqual(commands[1].watchpoints[0].allocation, allocation);
  commands.length = 0;
  await replaceWatchpoint(ask, undefined, allocation, "control");
  assert.deepEqual(commands.map((command) => command.operation), ["set_watchpoints", "list_watchpoints", "continue"]);
  for (const previous of [0, -1, NaN, "3"]) await assert.rejects(replaceWatchpoint(ask, previous, allocation, "control"));
  for (const mutation of ["wrong_stop_id", "wrong_spec", "extra_watchpoint", "completed"]) {
    const changed = async (command) => {
      const response = await ask(command);
      if (command.operation === "continue" && mutation === "wrong_stop_id") response.result.stop.watchpoint_id = 8;
      if (command.operation === "continue" && mutation === "completed") response.result.stop.reason = "completed";
      if (command.operation === "list_watchpoints" && mutation === "wrong_spec") response.result.watchpoints[0].spec = { ...spec, client_label: "other" };
      if (command.operation === "list_watchpoints" && mutation === "extra_watchpoint") response.result.watchpoints.push({ spec, watchpoint_id: 10 });
      return response;
    };
    await assert.rejects(replaceWatchpoint(changed, undefined, allocation, "control"));
  }
});

test("work, output and process bounds stay explicit and CLI requires exactly two directories", () => {
  assert.equal(LIMITS.page_items, 256); assert.equal(LIMITS.page_scanned, 256);
  assert.equal(LIMITS.pages * LIMITS.page_scanned, LIMITS.records);
  assert.equal(LIMITS.commands, 4096); assert.equal(LIMITS.discovery_steps, 256);
  assert.equal(LIMITS.transcript_bytes, 64 * 1024 * 1024);
  const script = join(dirname(fileURLToPath(import.meta.url)), "resource-query-lds-v5-smoke.mjs");
  for (const args of [[], ["one"], ["one", "two", "three"]]) {
    const result = spawnSync(process.execPath, [script, ...args], { encoding: "utf8", maxBuffer: 16384, timeout: 5000 });
    assert.equal(result.error, undefined); assert.notEqual(result.status, 0);
    assert.match(result.stderr, /usage: node scripts\/resource-query-lds-v5-smoke.mjs/u);
  }
});
