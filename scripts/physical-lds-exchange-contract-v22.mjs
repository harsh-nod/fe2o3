// Closed public source examples. Error boundaries match actual MIR39/KIR22 admission.
import assert from "node:assert/strict";
export const CASES_V22 = Object.freeze(Object.fromEntries(
  [
  [
    "one",
    null
  ],
  [
    "registers",
    null
  ],
  [
    "wrong-launch",
    "physical-lds-exchange requires exact authored launch128 and max_grid1"
  ],
  [
    "dynamic-grid",
    "physical-lds-exchange requires exact authored launch128 and max_grid1"
  ],
  [
    "frame-base",
    "physical-lds-exchange requires exact static_u32_frame(0,512,4,1)"
  ],
  [
    "frame-size",
    "physical-lds-exchange requires exact static_u32_frame(0,512,4,1)"
  ],
  [
    "frame-alignment",
    "physical-lds-exchange requires exact static_u32_frame(0,512,4,1)"
  ],
  [
    "frame-epoch",
    "physical-lds-exchange requires exact static_u32_frame(0,512,4,1)"
  ],
  [
    "missing-write-wait",
    "LDS exchange block/operation bounds"
  ],
  [
    "missing-barrier",
    "LDS exchange block/operation bounds"
  ],
  [
    "missing-read-wait",
    "LDS exchange block/operation bounds"
  ],
  [
    "wrong-peer",
    "LDS exchange xor64 actual localX"
  ],
  [
    "wrong-lds-address",
    "LDS exchange local write offset"
  ],
  [
    "wrong-store",
    "LDS exchange output requires exact ready peer LDS result"
  ],
  [
    "missing-vm",
    "LDS exchange block/operation bounds"
  ],
  [
    "wrong-carry",
    "LDS exchange pointer high provenance"
  ],
  [
    "foreign-input",
    "physical-lds-exchange begin requires exact root argument transport"
  ],
  [
    "foreign-marker",
    "physical-lds-exchange excludes helpers"
  ],
  [
    "mixed-marker",
    "physical-lds-exchange terminal roster contains a foreign or excessive marker"
  ]
].map(([name, refusal]) =>
    [name, Object.freeze({ feature: "physical-lds-exchange-" + name + "-v22", refusal })])));
export function caseV22(name) {
  assert.equal(typeof name, "string");
  assert.ok(Object.hasOwn(CASES_V22, name), "unknown or combined V22 source example");
  return CASES_V22[name];
}
export function timelyV22(start, now, limit) {
  assert.ok(Number.isFinite(start) && Number.isFinite(now) && Number.isFinite(limit));
  assert.ok(limit > 0 && now >= start && now - start < limit, "monotonic V22 workflow deadline");
}
export function sourceRefusalV22(name, status, signal, stderr, outputExists) {
  const { refusal } = caseV22(name);
  assert.notEqual(refusal, null, "positive source must not be refused");
  assert.ok(Number.isInteger(status) && status > 0, "actual unsuccessful compiler exit required");
  assert.equal(signal, null, "a signal is not source refusal");
  assert.ok(typeof stderr === "string" && Buffer.byteLength(stderr) <= 16 * 1024 * 1024);
  assert.ok(stderr.includes(refusal), "wrong exact V22 source refusal boundary");
  assert.equal(outputExists, false, "refused source created extraction output");
  return refusal;
}
export const CONDITIONS_V22 = Object.freeze([
  "full-EXEC", "initialized 128-u32 prefix (512 bytes)",
  "output requires 512 writable bytes",
  "input/output and output/kernarg disjointness remain runtime obligations",
  "live readable immutable 32-byte prefix aligned to 8 bytes",
  "one complete 128-invocation workgroup",
  "explicit write completion, workgroup publication and read completion",
  "no global happens-before or host admission is granted",
]);
