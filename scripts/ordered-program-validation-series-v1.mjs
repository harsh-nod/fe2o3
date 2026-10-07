#!/usr/bin/env node
// Read-only consistency checks. No source authentication, compiler admission,
// proof, artifact, hardware or performance-SLO authority is manufactured here.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { constants, openSync, closeSync, readSync, fstatSync, lstatSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const SAMPLE_PREFIX = "fe2o3 ordered-validation sample v1: ";
export const PREFIX = "fe2o3 ordered-validation series v1: ";
const STEPS = new Map([
  [1, [8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]],
  [3, [133, 307, 413, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]],
  [16, [0, 141, 323, 188, 321, 58, 64, 181, 60, 331, 73, 194, 56, 333, 16, 72]],
]);
const TOTAL = 35, CALIBRATION = 5, MEASURED = 30;
const STOP_NS = 60_000_000_000;
export const LIMITS = Object.freeze({ stdout: 256 * 1024, report: 65_536,
  sample: 2048, config: 65_536, baseline: 256 * 1024 });
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
function keys(value, names, label) {
  assert(value !== null && typeof value === "object" && !Array.isArray(value), label);
  assert.deepEqual(Object.keys(value).sort(), names.split(" ").sort(), label);
}
function integer(value, label, cap = Number.MAX_SAFE_INTEGER) {
  assert(Number.isSafeInteger(value) && value >= 0 && value <= cap, label);
}
function sha(value, label) { assert.match(value, /^[0-9a-f]{64}$/u, label); }
function compactJson(text, cap) {
  assert(Buffer.byteLength(text) <= cap, "JSON size");
  const value = JSON.parse(text);
  // The producer emits compact JSON with fixed ASCII keys/values and bounded
  // integer clocks. This refuses duplicate keys and ambiguous alternate text.
  assert.equal(JSON.stringify(value), text, "noncanonical/duplicate-key JSON");
  return value;
}
function checkIdentity(value, expected) {
  keys(value, "canonical_identity canonical_bytes semantic_identity source_inventory_identity source_preflight_identity canonical_executable_receipt_bytes call_correspondence_receipt_bytes retained_receipts_sum_bytes", "identity keys");
  for (const name of ["canonical_identity", "semantic_identity", "source_inventory_identity", "source_preflight_identity"]) sha(value[name], name);
  for (const name of ["canonical_bytes", "canonical_executable_receipt_bytes", "call_correspondence_receipt_bytes", "retained_receipts_sum_bytes"]) integer(value[name], name);
  assert.equal(value.canonical_bytes, expected.baseline_bytes);
  assert.equal(value.retained_receipts_sum_bytes,
    value.canonical_executable_receipt_bytes + value.call_correspondence_receipt_bytes);
}
function summary(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return { count: 30, p50_ns: sorted[14], p95_ns: sorted[28], maximum_ns: sorted[29] };
}

export function parseValidationSeries(stdout, expected) {
  assert.equal(typeof stdout, "string");
  assert(Buffer.byteLength(stdout) <= LIMITS.stdout, "stdout bound");
  keys(expected, "steps config_sha256 baseline_sha256 baseline_bytes", "expected keys");
  assert(STEPS.has(expected.steps), "closed 1/3/16 selector");
  sha(expected.config_sha256, "expected config");
  sha(expected.baseline_sha256, "expected baseline");
  integer(expected.baseline_bytes, "baseline size", LIMITS.baseline);
  assert(expected.baseline_bytes > 0);
  const events = [];
  for (const line of stdout.split("\n")) {
    if (line.startsWith(SAMPLE_PREFIX)) events.push(["sample", compactJson(line.slice(SAMPLE_PREFIX.length), LIMITS.sample)]);
    else if (line.startsWith(PREFIX)) events.push(["report", compactJson(line.slice(PREFIX.length), LIMITS.report)]);
    else assert(!line.includes(SAMPLE_PREFIX) && !line.includes(PREFIX), "misframed report");
  }
  assert.equal(events.length, TOTAL + 1, "all 35 samples and one final report required");
  assert.deepEqual(events.map(([kind]) => kind), [...Array(TOTAL).fill("sample"), "report"], "sample/report order");
  const report = events[TOTAL][1];
  keys(report, "schema series_mode steps descriptors calibration_samples measured_samples planned_transactions started_transactions completed_transactions compiler_invocations after_analysis_calls compiler_call_ns frontend_to_callback_ns callback_series_ns callback_stopping_budget_ns samples source_collection_summary validation_summary failure complete config_sha256 baseline_sha256 baseline_bytes generation_ns complete_owner_logical_bytes warm_stage target_compliance source_authentication_exported artifact_or_launch_authority", "report keys");
  assert.equal(report.schema, "fe2o3-ordered-validation-series-v1");
  assert.equal(report.series_mode, "same_callback_fresh_transactions_after_calibration");
  for (const name of Object.keys(expected)) assert.equal(report[name], expected[name], name);
  assert.deepEqual(report.descriptors, STEPS.get(expected.steps));
  assert.equal(report.calibration_samples, CALIBRATION);
  assert.equal(report.measured_samples, MEASURED);
  assert.equal(report.planned_transactions, TOTAL);
  assert.equal(report.started_transactions, TOTAL);
  assert.equal(report.completed_transactions, TOTAL);
  assert.equal(report.compiler_invocations, 1);
  assert.equal(report.after_analysis_calls, 1);
  assert.equal(report.callback_stopping_budget_ns, STOP_NS);
  assert.equal(report.complete, true, "partial series is unqualified");
  assert.equal(report.failure, null);
  assert.equal(report.generation_ns, null);
  assert.equal(report.complete_owner_logical_bytes, null);
  assert.equal(report.warm_stage, false);
  assert.equal(report.target_compliance, "unqualified");
  assert.equal(report.source_authentication_exported, false);
  assert.equal(report.artifact_or_launch_authority, false);
  for (const name of ["compiler_call_ns", "frontend_to_callback_ns", "callback_series_ns"]) integer(report[name], name);
  assert(report.callback_series_ns < STOP_NS, "callback stop budget");
  assert(report.frontend_to_callback_ns + report.callback_series_ns <= report.compiler_call_ns, "nested clock accounting");
  assert(Array.isArray(report.samples) && report.samples.length === TOTAL);
  let phaseTotal = 0;
  for (let index = 0; index < TOTAL; index++) {
    const envelope = events[index][1], sample = report.samples[index];
    keys(envelope, "schema sample", "sample envelope");
    assert.equal(envelope.schema, "fe2o3-ordered-validation-sample-v1");
    assert.deepEqual(envelope.sample, sample, "stream/aggregate exact sample join");
    keys(sample, "index phase source_collection_ns original_validation_transition_ns observation baseline_equal_while_owner_live owner_constructed owner_dropped_before_sample_publication observation_failure failure", "sample keys");
    assert.equal(sample.index, index, "unique ordered ordinal");
    assert.equal(sample.phase, index < CALIBRATION ? "calibration" : "measurement");
    assert.equal(sample.failure, null);
    assert.equal(sample.observation_failure, null);
    assert.equal(sample.baseline_equal_while_owner_live, true);
    assert.equal(sample.owner_constructed, true);
    assert.equal(sample.owner_dropped_before_sample_publication, true);
    integer(sample.source_collection_ns, "collection clock", STOP_NS);
    integer(sample.original_validation_transition_ns, "validation clock", STOP_NS);
    phaseTotal += sample.source_collection_ns + sample.original_validation_transition_ns;
    integer(phaseTotal, "phase sum");
    checkIdentity(sample.observation, expected);
    assert.deepEqual(sample.observation, report.samples[0].observation, "same actual identities/receipts");
  }
  assert(phaseTotal <= report.callback_series_ns, "phase clocks inside callback");
  assert.deepEqual(report.source_collection_summary,
    summary(report.samples.slice(CALIBRATION).map((sample) => sample.source_collection_ns)));
  assert.deepEqual(report.validation_summary,
    summary(report.samples.slice(CALIBRATION).map((sample) => sample.original_validation_transition_ns)));
  return Object.freeze({ schema: "fe2o3-ordered-validation-series-consistency-v1",
    steps: report.steps, retained_samples: TOTAL, measured_samples: MEASURED,
    calibration_excluded: true, source_collection: report.source_collection_summary,
    validation: report.validation_summary, generation_ns: null, warm_stage: false,
    source_authentication_exported: false, artifact_or_launch_authority: false,
    performance_budget_accepted: false });
}

function same(a, b) {
  return a.isFile() && b.isFile() && a.nlink === 1n && b.nlink === 1n &&
    ["dev", "ino", "mode", "size", "mtimeNs", "ctimeNs"].every((key) => a[key] === b[key]);
}
export function readBounded(path, cap) {
  assert.equal(resolve(path), path, "absolute exact path");
  assert.equal(realpathSync(path), path, "canonical path");
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const before = fstatSync(fd, { bigint: true });
    assert(same(before, lstatSync(path, { bigint: true })));
    assert(before.size > 0n && before.size <= BigInt(cap), "bounded regular file");
    const bytes = Buffer.alloc(Number(before.size));
    let offset = 0;
    while (offset < bytes.length) {
      const count = readSync(fd, bytes, offset, bytes.length - offset, null);
      assert(count > 0, "short read"); offset += count;
    }
    assert.equal(readSync(fd, Buffer.alloc(1), 0, 1, null), 0, "file grew");
    assert(same(before, fstatSync(fd, { bigint: true })) && same(before, lstatSync(path, { bigint: true })), "changed input");
    assert.equal(realpathSync(path), path);
    return bytes;
  } finally { closeSync(fd); }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert.equal(process.argv.length, 5, "usage: node ordered-program-validation-series-v1.mjs STDOUT CONFIG BASELINE");
  const [stdoutPath, configPath, baselinePath] = process.argv.slice(2);
  const stdout = readBounded(stdoutPath, LIMITS.stdout);
  const configBytes = readBounded(configPath, LIMITS.config);
  const baseline = readBounded(baselinePath, LIMITS.baseline);
  const config = JSON.parse(configBytes.toString("utf8"));
  keys(config, "schema steps rustc_args baseline_path baseline_bytes baseline_sha256", "config keys");
  assert.equal(config.schema, "fe2o3-ordered-validation-series-input-v1");
  assert.equal(config.baseline_path, baselinePath);
  assert.equal(config.baseline_bytes, baseline.length);
  assert.equal(config.baseline_sha256, hash(baseline));
  const result = parseValidationSeries(stdout.toString("utf8"), {
    steps: config.steps, config_sha256: hash(configBytes),
    baseline_sha256: hash(baseline), baseline_bytes: baseline.length,
  });
  // Re-read all exact inputs; a one-time parser is not continuous custody.
  assert.deepEqual(readBounded(stdoutPath, LIMITS.stdout), stdout);
  assert.deepEqual(readBounded(configPath, LIMITS.config), configBytes);
  assert.deepEqual(readBounded(baselinePath, LIMITS.baseline), baseline);
  process.stdout.write(`${JSON.stringify(result)}\n`);
}
