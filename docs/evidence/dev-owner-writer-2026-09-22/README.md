# Public Writer Lifecycle Correspondence

Status: completed developer qualification. Not a native admission qualification.

Implementation: `d6487e1866bb10c4b898d10e5dcdb3e815fcdf7f`.
Qualified source: `e83a6247de1ab97dc28ac19500200945788683a4`.
Frozen CPU baseline: `9b55f0c15895c0eb4d825410a2732542334e8225`.

## Scope

Journal `register_writer`, `lookup_reserved` and `abort_reserved` share execution
with the actual-type Verus root. Stable and producer owners share their explicit
registration and abort forwarding; reserved lookup is inherited through immutable
`Deref`, not a newly verified trait implementation. Lookup reuses the existing
shared Begin Reserved-identity leaf.

Raw correspondence is total, including malformed unrelated arenas, duplicate
free prefixes, physical writer slots above logical capacity, and non-issuable
already-Reserved keys. Exact error precedence and full actual-owner rejection
identity are preserved. Historical rejection contracts preserve sequence views,
not opaque historical Vec identity. Unchanged outer fields retain exact values.
The test-only access counter is outside normal-state identity.

Actual and historical operations execute independently. Projected producer status
is preserved unconditionally; this is not unconditional equality of malformed
status-query error results. Custody/issuance preservation requires the initial
combined `issued_producer_v1` invariant. Abort additionally binds its explicit
capacity observation to the ghost storage label. The pinned vstd Vec-length
lemma establishes the historical length cast; it supplies no capacity premise.

Three synthetic-start witnesses cover registration/lookup/abort/reuse/empty Begin,
batch enrollment/registration/nonempty Begin with member custody, and raw ID-zero
lookup with contrasting capacity observations. The last witness does not claim
that a literal empty Rust Vec has the supplied nonzero capacity. CPU tests also
exercise real Vec capacities and lazy observation ordering.

## Recording

- Raw actual-type root: 278 verified, zero errors.
- Whole historical root: 845 verified, zero errors, twice.
- All fourteen controls: normal scoped verification failures, no frontend or
  resource-limit failures.
- CPU: 952 unit tests and 27 doctests passed; 18 tests intentionally ignored.
- Formatting, all-target Clippy with warnings denied, and release test build passed.
- Pinned verifier closure and source brackets matched; every managed process group
  was absent at its terminal receipt.

The runner authenticates 339 inputs against the source commit. It reconstructs
the three ordinary-Rust owner roots from the frozen predecessor with only explicit
validated deltas, pins inherited proof sources and unchanged private runtime
adapters, and authenticates all external CPU source-shape inputs. Each staged
proof input is rehashed before its intended mutation. Stages are temporary.

The pinned verifier distribution is checked before and after proof execution.
Whole-root positives bracket fourteen scoped negative controls. Negative
diagnostics are captured and shape-checked first observations, not precommitted
diagnostic replay. CPU checks run in the source-hashed worktree after the proof
bracket; this is not a hermetic CPU-build or execution attestation.

`audit.py` checks the closed artifact roster, Git source identities, exact typed
results, command scopes, terminal/serial receipts, closure transcripts and CPU
totals. It does not rerun Verus, attest execution, or independently verify commit
signatures. Ten self-tests rehash corrupted packets and require rejection.
The audit and all ten self-tests passed. A bare Git object-database replay also
passed without a source checkout. Only the final source-bound campaign is archived;
exploratory runs and the interrupted preliminary recording are not qualification.

```sh
python3 -B docs/evidence/dev-owner-writer-2026-09-22/audit.py --repo . --selftest
```

## Boundaries

These are normal-state content proofs and instrumented frozen-baseline CPU tests.
Indexed work remains four accesses for successful registration, one for reserved
lookup, and three for successful abort, independent of arena size. These counters
are not elapsed-time measurements.

Fallible construction, physical allocation/capacity correspondence, unwind,
machine code, full owner reachability, Context/Worker integration, native pending
consumer admission and HIP/HSA performance remain separate. Gate 1 remains open.
No SSH host, GPU reservation or native benchmark was used for this checkpoint.
