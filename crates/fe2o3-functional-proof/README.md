# fe2o3-functional-proof

This crate owns the workload-neutral V2 wire format and strict import boundary
for functional-refinement proof receipts. It intentionally contains no Verus
runner, solver adapter, Pliron dependency, or compiler policy.

A receipt binds the safe-reference identity and MIR hash, optional source hash,
kernel subject and MIR hash, normalized obligation/effect IR hash, exact
Verus/solver/runtime identities, execution identity, result, covered boundary,
and signer. Import requires an external compiler policy, strict Ed25519
verification, an exact current binding, `Proved`, and the configured boundary.
Successful imports are move-only and replay-protected per importer.

Imported evidence covers only exact functional refinement at its named MIR
boundary. It never grants source-to-MIR, lowering, ISA, artifact, load, launch,
runtime, or hardware authority.
