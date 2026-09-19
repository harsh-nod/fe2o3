#!/usr/bin/env node
// Real Rust-source smoke workflow. No manufactured IR, hashes, or debug values.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
if (process.argv.length !== 3) throw new Error("usage: node scripts/authoring-v6-smoke.mjs NEW_OUTPUT_DIRECTORY");
const output = resolve(process.argv[2]);
mkdirSync(output); // Must be new: never overwrite an earlier run's evidence.
const bin = join(resolve(process.env.CARGO_TARGET_DIR ?? join(root, "target")), "debug");
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const save = (name, bytes) => writeFileSync(join(output, name), bytes, { flag: "wx" });
function run(name, args, input) {
  const result = spawnSync(join(bin, name), args, { cwd: root, input, encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 300000 });
  if (result.error || result.status !== 0) throw new Error(`${name} failed: ${result.error ?? result.stderr}`);
  return result.stdout;
}
run("fe2o3-export-sim", ["--crate", "fe2o3_fill", "--output", join(output, "fill-v6.fe2sim"), "--bundle-version", "6", "--target", "gfx942", "--target-dir", join(output, "extraction"), "--", "--package", "fe2o3-fill", "--lib"]);
const bundle = readFileSync(join(output, "fill-v6.fe2sim"));
const summaryText = run("fe2o3-author", ["inspect"], bundle);
const summary = JSON.parse(summaryText);
assert.equal(summary.canonical_kir_version, 11);
assert.equal(summary.authority.grants_production_resume, false);
save("snapshot.json", summaryText);
const pageText = run("fe2o3-author", ["operations", "--bundle-identity", summary.bundle_identity, "--start", "0", "--limit", "64"], bundle);
const page = JSON.parse(pageText);
assert.ok(page.operations.length > 0);
assert.ok(page.operations.some((operation) => operation.source_spans.length > 0));
save("operations.json", pageText);
const operation = page.operations.find((item) => item.source_spans.length > 0);
const selector = { bundle_identity: summary.bundle_identity, canonical_kir_digest: summary.canonical_kir_digest, target: summary.target, operations: [operation.coordinate] };
save("selector.json", `${JSON.stringify(selector)}\n`);
save("region.json", run("fe2o3-author", ["select", "--selector", JSON.stringify(selector)], bundle));
const stale = { ...selector, canonical_kir_digest: "0".repeat(64) };
const rejected = spawnSync(join(bin, "fe2o3-author"), ["select", "--selector", JSON.stringify(stale)], { input: bundle, encoding: "utf8", timeout: 30000 });
assert.equal(rejected.status, 1);
assert.equal(rejected.stdout, "");
assert.match(rejected.stderr, /stale or malformed exact canonical V11 identity/u);
save("stale-selection.txt", rejected.stderr);
const unsupported = spawnSync(join(bin, "fe2o3-author"), ["materialize", "--selector", JSON.stringify(selector), "--helper", "compute"], { input: bundle, encoding: "utf8", timeout: 30000 });
assert.equal(unsupported.status, 1);
assert.equal(unsupported.stdout, "");
assert.match(unsupported.stderr, /outside the diagnostic u32 gfx942 typed-ISA draft profile/u);
save("unsupported-promotion.txt", unsupported.stderr);

const request = JSON.parse(readFileSync(join(root, "scripts/quickstart/fill-request.json"), "utf8"));
const canary = "deadbeefcafebabe";
request.arguments[0].bytes += canary;
save("request.json", `${JSON.stringify(request)}\n`);
const simulation = run("fe2o3-kir-sim", ["--bundle-v6", join(output, "fill-v6.fe2sim"), "--request", join(output, "request.json")]);
save("simulation.json", simulation);
const simulated = JSON.parse(simulation);
assert.equal(simulated.status, "ok");
const expectedBytes = `0x${"00002a42".repeat(4)}${canary}`; // Independent 42.5f32 little-endian oracle.
assert.equal(simulated.arguments[0].value.bytes, expectedBytes, "all four output words and both canaries must match");
assert.equal(simulated.arguments[0].value.initialized, "0xffffff");
assert.equal(simulated.counts.invocations_executed, 4);
assert.equal(simulated.hardware_observed, false);
const commands = [
  { operation: "set_watchpoints", schema: "fe2o3-debug-request-v1", request_id: 1, expected_revision: 0, watchpoints: [{ client_label: "first-source-write", enabled: true, allocation: { ordinal: 1, generation: 0 }, byte_offset: 0, byte_len: 4, access: "write", timing: "after_commit" }] },
  { operation: "continue", schema: "fe2o3-debug-request-v1", request_id: 2, expected_revision: 1, max_events: 4096 },
  { operation: "step", schema: "fe2o3-debug-request-v1", request_id: 3, expected_revision: 2, direction: "forward", granularity: "operation", count: 1 },
  { operation: "read_memory", schema: "fe2o3-debug-request-v1", request_id: 4, expected_revision: 3, allocation: { ordinal: 1, generation: 0 }, byte_offset: 0, byte_len: 24 },
];
const commandText = commands.map((command) => JSON.stringify(command)).join("\n") + "\n";
save("debug-requests.jsonl", commandText);
const responseText = run("fe2o3-debug", ["sim", "--bundle-v6", join(output, "fill-v6.fe2sim"), "--request", join(output, "request.json")], commandText);
save("debug-responses.jsonl", responseText);
const responses = responseText.trim().split("\n").map((line) => JSON.parse(line));
assert.equal(responses.length, commands.length);
assert.ok(responses.every((response) => response.status === "ok"));
const memory = responses[3];
const expectedSnapshot = responses[2].result.snapshot.snapshot.anchor;
assert.deepEqual(memory.result.snapshot, expectedSnapshot);
assert.equal(memory.session.hardware_observed, false);
assert.equal(memory.result.snapshot.site.source.status, "resolved");
assert.equal(memory.result.snapshot.site.source.location.provenance, "compiler_bundle_bound");
assert.equal(memory.result.memory.availability.bytes, `0x00002a42${"00".repeat(12)}${canary}`);
save("resource-checkpoint.json", `${JSON.stringify({ schema: "fe2o3-resource-checkpoint-example-v1", origin: "ordinary_rust_source_export", source: "examples/fill/src/lib.rs", bundle_sha256: digest(bundle), response: memory, expectedSnapshot }, null, 2)}\n`);
save("receipt.json", `${JSON.stringify({ source: "examples/fill/src/lib.rs", source_sha256: digest(readFileSync(join(root, "examples/fill/src/lib.rs"))), bundle_sha256: digest(bundle), canonical_kir_digest: summary.canonical_kir_digest, bundle_identity: summary.bundle_identity, simulation_sha256: digest(simulation), debug_responses_sha256: digest(responseText), checks: ["ordinary_source_export", "bound_inspection", "region_selection", "stale_selection_rejection", "independent_four_word_oracle", "canaries", "source_bound_memory_checkpoint"], hardware_observed: false, typed_isa_frontend_readmission: "unavailable" }, null, 2)}\n`);
console.log(`Source inspection and CPU resource smoke checks passed; evidence: ${output}`);
