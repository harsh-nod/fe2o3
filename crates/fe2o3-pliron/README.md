# fe2o3-pliron

`fe2o3-pliron` is the issue #134 D0 boundary around the pinned Pliron
workspace. It provides:

- construction of a real Pliron `Context`;
- opaque process-local context identities backed by Pliron's private uniqued
  store rather than transferable auxiliary marker data;
- opaque operation handles whose upstream pointers remain in a private
  session registry;
- byte- and tree-guarded textual operation import that requires exact
  end-of-input and recursive verification before returning an owner handle;
- owner-scoped dialect-registration services with bounded typed actions;
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

The shell bounds registration count before collecting caller input, each
dialect hook to 64 typed registration actions, pass-plan count, names, and
diagnostics. A registration service accepts only types, attributes, and
operations owned by its assigned dialect namespace. Its private context and
dialect fields cannot be extracted or retained by safe callers. Operation
creation returns an opaque handle containing only a process-local owner
identity and session-local registry ID.
Textual operation import is a noncanonical construction bridge for dialect
integration. Input bytes, parser text, and printer output cannot become
artifact, cache, proof, publication, or runtime identities. The bridge bounds
input byte length and the delimiter syntax understood by the pinned, audited
parser set before parsing, then bounds the complete returned operation tree.
Successful import bytes are charged monotonically to the session because
interned parser data can outlive an erased operation. A parse or verification
rejection poisons the session because upstream arena allocation is not
transactional.

Every linked or registered Pliron `Parsable` implementation is trusted code at
this transitional boundary. The source and tree guards do not meter arbitrary
parser CPU time, temporary allocation, interning, comments, literals, or
private delimiter syntax. A deployment that adds a parser must audit or
process-contain it independently; the public registration action limit does
not make parser execution bounded. Consequently this API makes no resource
containment claim for an open third-party parser registry.
This bridge is transitional: a production compiler path that depends on a
printer/text round trip remains unsupported until it uses an owner-held typed
dialect construction service.
The corresponding upstream `Ptr<Operation>` remains in the private session
registry. Every query or erase authenticates the context anchor, owner, live
registry entry, and upstream pointee in that order. Erasure removes the
registry entry, so cloned stale handles cannot be revived by later arena
allocation. Handle identities and their debug representations are not
canonical data.

The crate does not execute a generic pass plan. Although an authenticated root
now exists, invoking an arbitrary caller-provided Pliron `Pass` would give that
pass a contextless pointer and `&mut Context`. Generic execution remains
disabled until compiler transformations use a sealed owner-aware service.
Hook and upstream diagnostic text is not copied into stable diagnostics; the
shell emits fixed fe2o3 codes and messages instead. Hook and upstream unwinds
remain contained by the session-construction boundary.

Context identities protect fe2o3-owned envelopes and results from being
validated against a different context, including when public Pliron auxiliary
marker boxes are moved between contexts. They do not add provenance to
upstream Pliron `Ptr<T>` values. Raw pointers remain contextless arena indexes
inside the Pliron trusted computing base and must not be exposed as a safe
cross-context capability. `ContextIdentity` intentionally hides its numeric
debug value but remains only process-local in-memory provenance; equality or
hashing must never become an artifact, cache, proof, publication, or runtime
identity.

## Remaining trusted surfaces

`DialectRegistrationHook` no longer receives `&mut Context`; all eight current
dialect adapters use `DialectRegistrationService`. Direct context access still
exists at these integration boundaries:

- `ensure_context_identity` and `require_context_identity` accept a caller-held
  Pliron context so existing owner-aware envelopes and detached services can
  authenticate their raw upstream handles.
- `with_context_mut` exists only behind the disabled-by-default
  `internal-test-context-access` feature for cross-crate conformance tests.
- Dialect crates retain legacy `register_dialect` or `register_mir_dialect`
  functions for direct-context lowering, bridge, compiler, and dialect-test
  callers that have not migrated to session-owned construction.
- Existing dialect builders, verifiers, and detached lowering services still
  accept raw contexts and, in some cases, contextless upstream pointers.

These are compiler-internal trusted-computing-base surfaces, not production
operation capabilities. The registration migration does not broaden them and
does not change the rule that ordinary session operation APIs return only
owner-authenticated handles.

## Upstream API findings

- Pliron constructs a real arena-owning context and automatically runs its
  linked context registrations. fe2o3 dialects are still registered through
  this crate's explicit hooks.
- `Dialect::register` is idempotent upstream. This shell preflights the complete
  registration list and rejects duplicate fe2o3 dialect declarations before
  constructing a context or invoking any hook.
- D0 plans only a flat list of leaf passes. Nested pass-manager values are
  rejected because their hidden children would evade the shell's pass-count
  bound. Pass name and manager inspection panics poison the plan and return a
  typed error. The plan is metadata only and cannot be executed through this
  crate.
- Pliron arena pointers and diagnostic display values are context-internal and
  are not suitable canonical identities. They are absent from manifests
  produced here.
- The v0.17.0 pass API has neither owner-aware operation handles nor a
  cooperative work or cancellation budget. fe2o3 now authenticates roots in
  its own session registry, but restoring execution still requires sealed pass
  access plus pass-work accounting or process containment.
- Upstream, hook, registration-input, pointer-access, and test-context callback
  unwinds are converted to typed errors under unwind-enabled builds. The
  registration action bound does not bound arbitrary computation inside a
  hook. As with all `catch_unwind` boundaries, `panic=abort`, allocator aborts,
  non-terminating code, and a panic in hostile destructor code cannot be
  converted into a Rust error.

The root workspace owns the exact Pliron revision so every dialect and lowering
crate resolves one audited upstream implementation. The selective
`pliron-llvm` dependency resolves that same revision and cannot broaden this
crate's authority boundary.
