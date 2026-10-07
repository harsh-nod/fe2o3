import assert from "node:assert/strict";
import test from "node:test";
import { PREFIX, SAMPLE_PREFIX, parseValidationSeries } from "./ordered-program-validation-series-v1.mjs";

// Entirely synthetic consistency fixtures: not measured compiler execution.
const expected = { steps: 3, config_sha256: "a".repeat(64),
  baseline_sha256: "b".repeat(64), baseline_bytes: 993 };
function fixture() {
  const identity = { canonical_identity: "c".repeat(64), canonical_bytes: 993,
    semantic_identity: "d".repeat(64), source_inventory_identity: "e".repeat(64),
    source_preflight_identity: "f".repeat(64), canonical_executable_receipt_bytes: 10,
    call_correspondence_receipt_bytes: 20, retained_receipts_sum_bytes: 30 };
  const samples = Array.from({ length: 35 }, (_, index) => ({
    index, phase: index < 5 ? "calibration" : "measurement",
    source_collection_ns: index < 5 ? 1000 : index + 1,
    original_validation_transition_ns: index < 5 ? 2000 : index + 101,
    observation: structuredClone(identity), baseline_equal_while_owner_live: true,
    owner_constructed: true, owner_dropped_before_sample_publication: true,
    observation_failure: null, failure: null,
  }));
  return { schema: "fe2o3-ordered-validation-series-v1",
    series_mode: "same_callback_fresh_transactions_after_calibration",
    ...expected, descriptors: [133, 307, 413, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    calibration_samples: 5, measured_samples: 30, planned_transactions: 35,
    started_transactions: 35, completed_transactions: 35,
    compiler_invocations: 1, after_analysis_calls: 1,
    compiler_call_ns: 1_000_000, frontend_to_callback_ns: 1000,
    callback_series_ns: 900_000, callback_stopping_budget_ns: 60_000_000_000,
    samples, source_collection_summary: { count: 30, p50_ns: 20, p95_ns: 34, maximum_ns: 35 },
    validation_summary: { count: 30, p50_ns: 120, p95_ns: 134, maximum_ns: 135 },
    failure: null, complete: true, generation_ns: null,
    complete_owner_logical_bytes: null, warm_stage: false, target_compliance: "unqualified",
    source_authentication_exported: false, artifact_or_launch_authority: false };
}
function stream(report, samples = report.samples) {
  return samples.map((sample) => SAMPLE_PREFIX + JSON.stringify({
    schema: "fe2o3-ordered-validation-sample-v1", sample,
  })).join("\n") + "\n" + PREFIX + JSON.stringify(report) + "\n";
}
test("complete synthetic series excludes five calibration points and preserves limitations", () => {
  const result = parseValidationSeries(stream(fixture()), expected);
  assert.equal(result.validation.p95_ns, 134);
  assert.equal(result.calibration_excluded, true);
  assert.equal(result.generation_ns, null);
  assert.equal(result.warm_stage, false);
  assert.equal(result.performance_budget_accepted, false);
});
for (const [name, mutate] of [
  ["old schema", (r) => { r.schema = "fe2o3-ordered-stage-measurement-v1"; }],
  ["partial", (r) => { r.complete = false; }],
  ["failure", (r) => { r.failure = "compiler_fatal"; }],
  ["wrong selector", (r) => { r.steps = 1; }],
  ["descriptor drift", (r) => { r.descriptors[1] ^= 1; }],
  ["wrong baseline", (r) => { r.baseline_sha256 = "0".repeat(64); }],
  ["wrong config", (r) => { r.config_sha256 = "0".repeat(64); }],
  ["extra field", (r) => { r.proof = true; }],
  ["fake generation", (r) => { r.generation_ns = 0; }],
  ["fake warm stage", (r) => { r.warm_stage = true; }],
  ["authority", (r) => { r.artifact_or_launch_authority = true; }],
  ["repeated callback", (r) => { r.after_analysis_calls = 2; }],
  ["budget extension", (r) => { r.callback_stopping_budget_ns++; }],
  ["expired callback", (r) => { r.callback_series_ns = 60_000_000_000; }],
  ["unnested clocks", (r) => { r.compiler_call_ns = 100; }],
  ["drop measurement", (r) => { r.samples.pop(); r.completed_transactions--; }],
  ["missing transaction", (r) => { r.started_transactions--; }],
  ["duplicate index", (r) => { r.samples[34].index = 33; }],
  ["reorder", (r) => { [r.samples[3], r.samples[4]] = [r.samples[4], r.samples[3]]; }],
  ["phase substitution", (r) => { r.samples[5].phase = "calibration"; }],
  ["hidden refusal", (r) => { r.samples[7].failure = "validation"; }],
  ["missing clock", (r) => { r.samples[0].source_collection_ns = null; }],
  ["fractional clock", (r) => { r.samples[8].original_validation_transition_ns = 1.5; }],
  ["unsafe integer", (r) => { r.samples[8].original_validation_transition_ns = Number.MAX_SAFE_INTEGER + 1; }],
  ["source identity drift", (r) => { r.samples[7].observation.source_inventory_identity = "0".repeat(64); }],
  ["receipt sum drift", (r) => { r.samples[7].observation.retained_receipts_sum_bytes++; }],
  ["live owner", (r) => { r.samples[7].owner_dropped_before_sample_publication = false; }],
  ["baseline unchecked", (r) => { r.samples[7].baseline_equal_while_owner_live = false; }],
  ["calibration in summary", (r) => { r.validation_summary.maximum_ns = 2000; }],
  ["wrong quantile", (r) => { r.validation_summary.p95_ns = 133; }],
]) {
  test("refuses " + name, () => {
    const report = fixture(); mutate(report);
    assert.throws(() => parseValidationSeries(stream(report), expected));
  });
}
test("raw prefix and final aggregate must agree byte-semantically", () => {
  const report = fixture(), samples = structuredClone(report.samples);
  samples[5].source_collection_ns++;
  assert.throws(() => parseValidationSeries(stream(report, samples), expected));
});
test("missing duplicate reordered and post-report samples refuse", () => {
  const original = stream(fixture()).trimEnd().split("\n");
  for (const lines of [
    original.slice(1),
    [original[0], ...original],
    [original[1], original[0], ...original.slice(2)],
    [...original.slice(1), original[0]],
  ]) assert.throws(() => parseValidationSeries(lines.join("\n") + "\n", expected));
});
test("duplicate JSON keys and misframing refuse", () => {
  const raw = stream(fixture());
  const duplicate = raw.replace('"index":0', '"index":9,"index":0');
  assert.throws(() => parseValidationSeries(duplicate, expected));
  assert.throws(() => parseValidationSeries("noise " + raw, expected));
});
test("oversized input and unknown expected selectors refuse", () => {
  assert.throws(() => parseValidationSeries("x".repeat(256 * 1024 + 1), expected));
  assert.throws(() => parseValidationSeries(stream(fixture()), { ...expected, steps: 2 }));
});
