# fe2o3-pliron

`fe2o3-pliron` is the issue #134 D0 boundary around the pinned Pliron
workspace. It provides:

- construction of a real Pliron `Context`;
- opaque process-local context identities backed by Pliron's private uniqued
  store rather than transferable auxiliary marker data;
- explicit, bounded dialect-registration hooks;
- deterministic, bounded pass plans over real Pliron `Pass` values.

The dependency is pinned to Pliron v0.17.0 commit
`2610651306ea3ba670f68d5d8b1e1159bcd521ed`. The `pliron-derive` dependency
used by `pliron` is sourced from that same Git workspace revision. This crate
does not construct or lower LLVM operations. The workspace now pins
`pliron-llvm` at the same revision for the isolated dialect smoke crate, and
the first #144 slice defines a Pliron-independent canonical LLVM handoff.

The target integration permits `pliron-llvm` only for transient `llvm.*`
dialect construction, transformation, and verification.
Ordinary compiler crates must use `default-features = false` so the optional
`llvm-sys` dependency is not linked into their processes. fe2o3-owned bounded
canonical records, identities, receipts, and evidence form the finalizer
handoff; Pliron handles, printer text, and diagnostics do not. The isolated
pinned upstream LLVM 22.1.8 target machine and in-process LLD remain the sole
machine-code and HSACO authority.

## Boundary

This crate does not define fe2o3 dialect operations, canonicalize IR, select a
production compiler, publish artifacts, grant proof or launch authority, or
use COMGR. Pliron pointers, arena identities, printer text, and diagnostics
are never used as canonical fe2o3 identities.

The shell bounds registration count, pass-plan count, names, and diagnostics.
It does not execute a generic pass plan. Upstream Pliron `Ptr<T>` values are
contextless arena indexes, so the previous safe execution API could not
authenticate its root against the supplied session. Generic execution remains
disabled until issue #140 provides an owner-aware handle. Hook and upstream
diagnostic text is not copied into stable diagnostics; the shell emits fixed
fe2o3 codes and messages instead.

Context identities protect fe2o3-owned envelopes and results from being
validated against a different context, including when public Pliron auxiliary
marker boxes are moved between contexts. They do not add provenance to
upstream Pliron `Ptr<T>` values. Raw pointers remain contextless arena indexes
inside the Pliron trusted computing base and must not be exposed as a safe
cross-context capability.

## Upstream API findings

- Pliron constructs a real arena-owning context and automatically runs its
  linked context registrations. fe2o3 dialects are still registered through
  this crate's explicit hooks.
- `Dialect::register` is idempotent upstream. This shell preflights the complete
  registration list and rejects duplicate fe2o3 dialect declarations before
  constructing a context or invoking any hook.
- D0 plans only a flat list of leaf passes. Nested pass-manager values are
  rejected because their hidden children would evade the shell's pass-count
  bound. The plan is metadata only and cannot be executed through this crate.
- Pliron arena pointers and diagnostic display values are context-internal and
  are not suitable canonical identities. They are absent from manifests
  produced here.
- The v0.17.0 pass API has neither owner-aware operation handles nor a
  cooperative work or cancellation budget. Restoring execution requires both
  authenticated roots and future pass-work accounting or process containment.

The root workspace owns the exact Pliron revision so every dialect and lowering
crate resolves one audited upstream implementation. The selective
`pliron-llvm` dependency resolves that same revision and cannot broaden this
crate's authority boundary.
