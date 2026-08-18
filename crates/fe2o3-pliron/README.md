# fe2o3-pliron

`fe2o3-pliron` is the issue #134 D0 boundary around the pinned Pliron
workspace. It provides:

- construction of a real Pliron `Context`;
- explicit, bounded dialect-registration hooks;
- deterministic, bounded pass plans over real Pliron `Pass` values;
- verification before and after every pass; and
- deterministic stage-attempt receipts that explicitly grant no authority.

The dependency is pinned to Pliron v0.17.0 commit
`2610651306ea3ba670f68d5d8b1e1159bcd521ed`. The `pliron-derive` dependency
used by `pliron` is sourced from that same Git workspace revision. This crate
does not use `pliron-llvm` yet because D0 does not construct or lower LLVM
operations.

## Boundary

This crate does not define fe2o3 dialect operations, canonicalize IR, select a
production compiler, publish artifacts, grant proof or launch authority, or
use COMGR. Pliron pointers, arena identities, printer text, and diagnostics
are never used as canonical fe2o3 identities.

The shell bounds registration count, pass count, names, and diagnostics. An
arbitrary in-process pass can still consume unbounded time or memory; enforcing
pass-work and wall-clock budgets requires cooperative pass accounting or
worker containment in a later stage. A pass or verification failure poisons
the session so partially transformed IR cannot be reused through this API.
Hook and upstream diagnostic text is not copied into stable diagnostics; the
shell emits fixed fe2o3 codes and messages instead.

## Upstream API findings

- Pliron constructs a real arena-owning context and automatically runs its
  linked context registrations. fe2o3 dialects are still registered through
  this crate's explicit hooks.
- `Dialect::register` is idempotent upstream. This shell preflights the complete
  registration list and rejects duplicate fe2o3 dialect declarations before
  constructing a context or invoking any hook.
- Pliron exposes verifier hooks through its pass manager. This shell invokes
  `verify_operation` explicitly around each pass so pre-pass and post-pass
  failure have distinct stable diagnostics, then delegates execution through
  the real Pliron `PassManager` path.
- D0 accepts only a flat list of leaf passes. Nested pass-manager values are
  rejected because their hidden children would evade this shell's pass-count
  bound and explicit verification receipts.
- Pliron arena pointers and diagnostic display values are context-internal and
  are not suitable canonical identities. They are absent from manifests and
  receipts produced here.
- The v0.17.0 pass API has no cooperative work or cancellation budget. The
  current count bounds do not substitute for future pass-work accounting or
  process containment.

The root workspace owns the exact Pliron revision so every dialect and lowering
crate resolves one audited upstream implementation.
