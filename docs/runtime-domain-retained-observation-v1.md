# Domain Retained Observation

`DomainAccount::matches_retained_charge` now shares its actual post-lock
body with `domain_retained_observation_v1.rs`. Arc identity and mutex acquisition
remain in the unchanged production caller. The shared helper borrows the actual
locked node and record slices; it does not receive precomputed ancestry or
credit-validity booleans.

## Theorem

The result is true exactly when the stored poison flag is clear, the selected
domain has a valid generation-bound path to the exact root for its three- or
four-level profile, the record slot exists and is occupied, its leaf matches
the selected domain, and its retained-credit record matches the requested
nonzero owner and all nineteen charge coordinates. Zero charges remain valid.

The proof reuses the production node declarations, bounded path traversal and
retained-record expression. Its seven-file executable closure rechecks the
path invariants instead of replacing traversal with an assumed boolean.
The helper is read-only. This establishes a predicate over that borrowed state,
not a resource capability or a coherent observation across separate calls.

Pinned vstd slice, array and structural-equality specifications, Rust derive
lowering and Rust/Verus compilation remain trusted. There is no local assumed
proof body. Arc identity, successful locking, mutex-poison acquisition semantics,
cross-call freshness, ledger conservation, token custody, Context admission,
native execution and compiler/ISA correctness remain outside this theorem.

## Qualification

Signed candidate `3e8a02904`, integrated through `8b87713bb` and `a30ff758e`,
passes twenty-five stages with three full `--no-cheating` runs of twenty-one
overlapping obligations, including exact closure relocation. All fifteen
actual-body logical mutations are rejected. Both 190-file verifier-release
checks, signed-source continuity and all twenty-five fresh same-namespace
process-group closure checks pass. Evidence remains local under
`fe2o3-domain-retained-observer-qualification-20260930-retained`.

The prior campaign remains rejected: its bounds-removal mutation produced a
real indexing-precondition failure whose exact diagnostic was not in the frozen
classifier. The signed child adds that exact diagnostic to this observer's
classifier only, with negative controls for status, level, location and nearby
messages. Production and proof bytes are unchanged; the entire campaign was
rerun. Compiler failures and timeouts are not accepted as logical negatives.

A separate pre-signing run passes all seventy-six accounting CPU tests. Its
fifteen accounting Rust inputs, seven proof files and build inputs are bound
byte-for-byte to the signed child; changed metadata is explicitly inventoried.
This is not a fresh signed-source CPU execution or a merged-runtime result.

The earlier record-predicate and R75 source manifests include the new shared
body. Their executable closures, expected counts and mutation policies remain
unchanged. Lightweight calibration passes, without a new standalone campaign
for those roots. No A0-A7 exit, native execution or performance result follows
from this component qualification.
