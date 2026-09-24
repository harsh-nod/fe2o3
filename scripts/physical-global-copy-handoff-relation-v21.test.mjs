import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { observedHandoffRelationV21 as observe, RELATION_PREFIX_V21 as prefix } from "./physical-global-copy-handoff-relation-v21.mjs";
const llvm = Buffer.from("; inert test-only canonical text\n");
const handoff = Buffer.from("inert test-only handoff bytes, not an admitted wire object");
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const example = () => ({
  schema: "fe2o3-physical-global-copy-inert-handoff-relation-v21",
  canonical_identity: "1".repeat(64), canonical_llvm_sha256: hash(llvm),
  handoff_sha256: hash(handoff), descriptor_sha256: "2".repeat(64),
  runtime_conditions_discharged: false, source_custody_exported: false, host_admitted: false,
  protected_finalizer_admitted: false, native_llvm_executed: false, hardware_observed: false,
  grants_artifact_or_launch_authority: false,
});
const line = row => prefix + JSON.stringify(row) + "\n";
test("one closed diagnostic relation joins observed hashes without wire or source authority", () => {
  const row = observe("ordinary diagnostic\n" + line(example()), llvm, handoff);
  assert.deepEqual(row, example());
  assert.ok(Object.isFrozen(row));
  assert.equal(row.source_custody_exported, false);
});
test("missing, repeated, truncated, or oversized diagnostic relations refuse", () => {
  for (const stderr of ["", line(example()) + line(example()), prefix + "{\n", prefix + " ".repeat(4097)])
    assert.throws(() => observe(stderr, llvm, handoff));
  assert.throws(() => observe(" ".repeat(16 * 1024 * 1024 + 1), llvm, handoff));
});
test("unknown, duplicate, noncanonical, nested, and malformed fields refuse", () => {
  const unknown = { ...example(), future_field: false };
  const missing = example(); delete missing.schema;
  const nested = { ...example(), canonical_identity: { digest: "1".repeat(64) } };
  for (const row of [unknown, missing, nested, null, []])
    assert.throws(() => observe(line(row), llvm, handoff));
  const duplicate = JSON.stringify(example()).replace("{", '{"schema":"other",');
  assert.throws(() => observe(prefix + duplicate + "\n", llvm, handoff));
  assert.throws(() => observe(prefix + " " + JSON.stringify(example()) + "\n", llvm, handoff));
});
test("each authority claim and digest substitution refuses", () => {
  for (const key of ["runtime_conditions_discharged", "source_custody_exported", "host_admitted",
    "protected_finalizer_admitted", "native_llvm_executed", "hardware_observed", "grants_artifact_or_launch_authority"])
    assert.throws(() => observe(line({ ...example(), [key]: true }), llvm, handoff));
  for (const key of ["canonical_identity", "canonical_llvm_sha256", "handoff_sha256", "descriptor_sha256"])
    assert.throws(() => observe(line({ ...example(), [key]: "A".repeat(64) }), llvm, handoff));
  for (const key of ["canonical_llvm_sha256", "handoff_sha256"])
    assert.throws(() => observe(line({ ...example(), [key]: "f".repeat(64) }), llvm, handoff));
});
test("wrong or unbounded output bytes do not satisfy the relation", () => {
  assert.throws(() => observe(line(example()), Buffer.concat([llvm, Buffer.from("\n")]), handoff));
  assert.throws(() => observe(line(example()), llvm, Buffer.concat([handoff, Buffer.from("\n")])));
  for (const bytes of [Buffer.alloc(0), Buffer.alloc(64 * 1024 + 1), "text"])
    assert.throws(() => observe(line(example()), bytes, handoff));
  for (const bytes of [Buffer.alloc(0), Buffer.alloc(1024 * 1024 + 1), "bytes"])
    assert.throws(() => observe(line(example()), llvm, bytes));
});
