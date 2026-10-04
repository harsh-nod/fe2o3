import assert from "node:assert/strict";
import test from "node:test";
import { LIMITS, scalePrecheck, summarize } from "./resource-query-scale.mjs";

test("only bounded pinned-fixture scales are admitted", () => {
  for (const size of [4, 64, 128]) {
    const result = scalePrecheck(size);
    assert.equal(result.status, "admitted_by_script_precheck");
    assert.ok(result.estimated_records <= 8192);
    assert.ok(result.estimated_capture_bytes <= 64 * 1024 * 1024);
    assert.equal(result.allocation_bytes, size * 4 + 8);
  }
  assert.deepEqual(scalePrecheck(256), { invocations: 256, allocation_bytes: 1032,
    estimated_records: 8768, estimated_capture_bytes: 80876032,
    status: "not_run_budget_exceeded", reason: "estimated_records_exceed_8192" });
  for (const invalid of [0, -1, 1.5, 32, 8192, Number.MAX_SAFE_INTEGER, NaN, "128"]) assert.throws(() => scalePrecheck(invalid));
  assert.equal(LIMITS.page_items, 256);
  assert.equal(LIMITS.page_scanned, 256);
  assert.equal(LIMITS.per_case_timeout_ms, 120000);
});

test("bounded nearest-rank statistics retain full raw inputs", () => {
  const original = [10, 1, 3, 2];
  assert.deepEqual(summarize(original), { samples: 4, min: 1, p50: 2, p95: 10, max: 10, mean: 4 });
  assert.deepEqual(original, [10, 1, 3, 2]);
  for (const invalid of [[], [NaN], [Infinity], [-1], Array(3), Array(8193).fill(0)]) assert.throws(() => summarize(invalid));
});
