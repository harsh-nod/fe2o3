// Bounded presentation-only relation, not a handoff/source/runtime decoder.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
export const RELATION_PREFIX_V21 = "FE2O3_PHYSICAL_GLOBAL_COPY_HANDOFF_RELATION_V21 ";
const schema = "fe2o3-physical-global-copy-inert-handoff-relation-v21";
const digests = ["canonical_identity", "canonical_llvm_sha256", "handoff_sha256", "descriptor_sha256"];
const unavailable = ["runtime_conditions_discharged", "source_custody_exported", "host_admitted",
  "protected_finalizer_admitted", "native_llvm_executed", "hardware_observed", "grants_artifact_or_launch_authority"];
const keys = ["schema", ...digests, ...unavailable].sort();
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
export function observedHandoffRelationV21(stderr, canonicalLLVM, handoff) {
  assert.ok(typeof stderr === "string" && stderr.length <= 16 * 1024 * 1024 &&
    Buffer.byteLength(stderr, "utf8") <= 16 * 1024 * 1024, "bounded diagnostic stderr");
  assert.ok(Buffer.isBuffer(canonicalLLVM) && canonicalLLVM.length > 0 && canonicalLLVM.length <= 64 * 1024, "bounded canonical LLVM");
  assert.ok(Buffer.isBuffer(handoff) && handoff.length > 0 && handoff.length <= 1024 * 1024, "bounded inert handoff");
  const rows = stderr.split("\n").filter(line => line.startsWith(RELATION_PREFIX_V21));
  assert.equal(rows.length, 1, "exactly one V21 diagnostic handoff relation");
  const raw = rows[0].slice(RELATION_PREFIX_V21.length);
  assert.ok(raw.length <= 4096, "bounded relation record");
  const row = JSON.parse(raw);
  assert.ok(row !== null && typeof row === "object" && !Array.isArray(row), "flat relation object");
  // The producer emits compact JSON. Round-trip equality also refuses duplicate
  // keys/noncanonical whitespace/escapes rather than interpreting ambiguous rows.
  assert.equal(JSON.stringify(row), raw, "canonical diagnostic JSON spelling");
  assert.deepEqual(Object.keys(row).sort(), keys, "closed diagnostic relation fields");
  assert.equal(row.schema, schema);
  for (const key of digests) assert.match(row[key], /^[0-9a-f]{64}$/u, "exact digest: " + key);
  for (const key of unavailable) assert.equal(row[key], false, "unavailable authority: " + key);
  assert.equal(row.canonical_llvm_sha256, hash(canonicalLLVM), "same unchanged canonical LLVM diagnostic hash");
  assert.equal(row.handoff_sha256, hash(handoff), "same serialized inert handoff diagnostic hash");
  // The canonical identity and descriptor digest remain declared observations.
  // This comparison never decodes a source owner or authenticates the compiler.
  return Object.freeze(row);
}
