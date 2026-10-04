import test from "node:test";
import assert from "node:assert/strict";
import { CASES_V22, caseV22, timelyV22, sourceRefusalV22, CONDITIONS_V22 } from "./physical-lds-exchange-contract-v22.mjs";

test("exact two positive and seventeen negative source controls", () => {
  assert.equal(Object.keys(CASES_V22).length, 19);
  assert.deepEqual(Object.entries(CASES_V22).filter(([, row]) => row.refusal === null).map(([key]) => key), ["one", "registers"]);
  for (const [name, row] of Object.entries(CASES_V22)) {
    assert.equal(caseV22(name), row);
    assert.equal(row.feature, "physical-lds-exchange-" + name + "-v22");
    assert.ok(Object.isFrozen(row));
  }
});
test("unknown, combined, inherited and traversal cases refuse", () => {
  for (const name of ["", "one,registers", "__proto__", "constructor", "../one", "ONE", "native", null])
    assert.throws(() => caseV22(name));
});
test("every negative requires its actual source boundary", () => {
  for (const [name, row] of Object.entries(CASES_V22).filter(([, r]) => r.refusal !== null)) {
    assert.equal(sourceRefusalV22(name, 101, null, "prefix: " + row.refusal + "\n", false), row.refusal);
    for (const other of Object.values(CASES_V22).filter(r => r.refusal && r.refusal !== row.refusal))
      assert.throws(() => sourceRefusalV22(name, 101, null, other.refusal, false));
    for (const generic of ["unsupported KIR operation", "actual compiler process failed", ""])
      assert.throws(() => sourceRefusalV22(name, 101, null, generic, false));
  }
});
test("signal, missing exit, success and created output cannot count as source refusal", () => {
  const fragment = caseV22("wrong-launch").refusal;
  for (const status of [0, null, undefined, -1, 1.5])
    assert.throws(() => sourceRefusalV22("wrong-launch", status, null, fragment, false));
  assert.throws(() => sourceRefusalV22("wrong-launch", 101, "SIGKILL", fragment, false));
  assert.throws(() => sourceRefusalV22("wrong-launch", 101, null, fragment, true));
  for (const name of ["one", "registers"])
    assert.throws(() => sourceRefusalV22(name, 101, null, fragment, false));
});
test("removed rows retain earlier block/operation refusal, not invented wait diagnostic", () => {
  for (const name of ["missing-write-wait", "missing-barrier", "missing-read-wait", "missing-vm"]) {
    assert.equal(caseV22(name).refusal, "LDS exchange block/operation bounds");
    for (const later of ["LDS exchange exact initial32 native rows", "LDS exchange requires write LGKM completion"])
      assert.throws(() => sourceRefusalV22(name, 101, null, later, false));
  }
});
test("foreign argument uses actual root transport refusal and not panic", () => {
  assert.equal(caseV22("foreign-input").refusal, "physical-lds-exchange begin requires exact root argument transport");
  for (const old of ["physical-lds-exchange source contains a foreign argument assignment", "device code reaches a panic path"])
    assert.throws(() => sourceRefusalV22("foreign-input", 101, null, old, false));
});
test("bounded rejection text", () => {
  const fragment = caseV22("wrong-launch").refusal;
  assert.throws(() => sourceRefusalV22("wrong-launch", 101, null, fragment + "x".repeat(16 * 1024 * 1024), false));
  assert.throws(() => sourceRefusalV22("wrong-launch", 101, null, {}, false));
});
test("post-observation deadlines reject exact and late positives", () => {
  for (const limit of [300000, 900000]) {
    timelyV22(100, 100 + limit - 0.01, limit);
    assert.throws(() => timelyV22(100, 100 + limit, limit));
    assert.throws(() => timelyV22(100, 101 + limit, limit));
  }
});
test("invalid or reversed clocks refuse", () => {
  for (const [start, now, limit] of [[1, 0, 5], [0, NaN, 5], [0, Infinity, 5], [0, 1, 0], [NaN, 1, 5]])
    assert.throws(() => timelyV22(start, now, limit));
});
test("normal source diagnostic conditions retain global, kernarg and LDS requirements", () => {
  assert.equal(CONDITIONS_V22.length, 8);
  assert.ok(CONDITIONS_V22.some(s => s.includes("512 writable")));
  assert.ok(CONDITIONS_V22.some(s => s.includes("32-byte")));
  assert.ok(CONDITIONS_V22.some(s => s.includes("128-invocation")));
  assert.ok(CONDITIONS_V22.some(s => s.includes("workgroup publication")));
  assert.ok(CONDITIONS_V22.some(s => s.includes("no global happens-before")));
});
