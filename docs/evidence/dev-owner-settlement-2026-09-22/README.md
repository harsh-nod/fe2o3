# Public Settlement Execution

Status: completed developer execution qualification, not Gate 1 closure or
native runtime qualification.

Qualified source: `ba643eec26c8f8cc1598af56aff03bc5388e59b5`.

Frozen CPU baseline: `269bb2c3d57b1221ee5e8871bf935d61dbacdc27`.

## Scope

Journal, stable-reader and producer-reader `settle_success` / `settle_no_effect`
now share executable wrappers with the actual-owner Verus universe. Journal
preflight and orchestration also share code, reusing the unchanged retained-chain,
return-storage, scratch and commit leaves. Evidence remains an inert premise,
not authenticated backend completion.

Raw contracts require no caller custody, issuability or success premise. They
preserve exact error precedence and whole actual-owner identity on rejection.
Success preserves outer reader/producer fields, journal scalars and allocation-free
storage exactly; scratch contents are restored without asserting opaque Vec
identity after mutation. Sequential prefix semantics remain intact.
The test-only access counter is outside normal-state identity.

Both physical-capacity expressions are evaluated exactly once, writer then member,
after header, evidence and complete-chain validation. Both are observed before
return admission, even when logical writer headroom is insufficient. The proof
receives these as scalar observations, not physical allocation guarantees.

The nonempty raw witness exercises both outcomes with nonzero prior lineages,
dirty scratch tails, malformed unrelated metadata and a non-issuable stored writer
key. It also checks headroom rejection and replay rejection. It is not a
constructor-reachable lifecycle witness.

Frozen CPU comparisons execute all three old owner layers on the same allocation
as the candidate. Success restoration touches only the saved validated chain,
selected writer and scratch prefix, and truncates returned free-stack suffixes.
Watermark and reserved count are asserted unchanged, never repaired. Rejected
malformed chains are not traversed for restoration. Existing tests retain coverage
of physical headroom, raw faults, repeated attempts, and unrelated producer states.

## Recording

- Raw actual-owner root: 306 verified, zero errors, twice.
- Separate unchanged writer historical root: 845 verified, zero errors.
- Eleven controls: expected scoped logical failures, no frontend/resource failures.
- CPU: 956 unit tests and 27 doctests passed; 18 intentionally ignored.
- Formatting, warnings-denied all-target Clippy and release test build passed.
- All 351 source inputs and the pinned verifier closure matched before/after;
  all 21 managed process groups were absent at their terminal receipts.

The source-bound runner is `crates/fe2o3-runtime-model/verus/check-owner-settlement.py`.
It authenticates the frozen chain, exact ordinary-Rust adapters and raw-root
envelope, inherited proofs and unchanged private runtime dependencies. It checks
the pinned verifier distribution before and after staged proof runs. CPU checks
run in a source-hashed worktree; they are not hermetic execution attestations.

Two full raw-root positives bracket eleven scoped mutation controls. A separate
unchanged writer historical root is a regression check only: its count must not
be described as settlement historical correspondence. Negative diagnostics are
captured first observations checked for scoped logical failure, not precommitted
diagnostic replay.

`audit.py` checks artifact hashes, exact Git source identities, result types,
command scopes, serial terminal receipts, verifier closure and CPU counts. It
does not rerun verification, attest execution or independently verify signatures.
Thirteen rehashed corruption self-tests exercise the integrity checks.
The audit and all thirteen self-tests passed, as did a bare Git object-database
replay without a source checkout. Only the final source-bound campaign is
archived. Exploratory runs are not qualification. All 44 recorded process groups
across exploratory and final runs were independently absent before removing the
owned scratch directory; no other job or shared-host directory was removed.

```sh
python3 -B docs/evidence/dev-owner-settlement-2026-09-22/audit.py --repo . --selftest
```

## Remaining Work

Settlement's paired historical execution, conditional issued-custody preservation,
retained-status bridge and reachable enrollment/Begin/settlement witnesses remain
open. So do fallible construction, scalar enrollment, retirement/disposal,
physical storage/unwind refinement and native Context integration. Gate 1 remains
open and native pending-consumer admission remains disabled.

Successful settlement retains `9k + 3` counted indexed accesses for `k` members;
that is an operation count, not a timing measurement. No SSH host or GPU was used
for this checkpoint. No HIP/HSA parity or performance gain is claimed.
