# fe2o3-functional-proof

This crate owns the workload-neutral V2 wire format and strict import boundary
for functional-refinement proof receipts. It intentionally contains no Verus
runner, solver adapter, Pliron dependency, or compiler policy.

A receipt binds the safe-reference identity and MIR hash, optional source hash,
kernel subject and MIR hash, normalized obligation/effect IR hash, exact
Verus/solver/runtime identities, execution identity, result, covered boundary,
and signer. Import requires an external policy, strict Ed25519
verification, an exact current binding, `Proved`, and the configured boundary.
Successful imports are move-only and replay-protected per importer.

An imported value says only that a receipt passed those checks under the policy
and expected identities supplied to this crate. It is inert with respect to
compiler authority: it does not establish that the policy, MIR hashes, or
effect formulas came from rustc custody. Production admission requires a
separate compiler-private join to retained rustc MIR and compiler configuration.

At that boundary, Verus proves the compiler-derived effect formulas conditional
on the trusted MIR-to-effect extractor and the exact integer, floating-point,
layout, and memory model emitted by the proof generator. This is not a full MIR
operational-semantics theorem. A receipt never grants source-to-MIR, lowering,
ISA, artifact, load, launch, runtime, or hardware authority.
