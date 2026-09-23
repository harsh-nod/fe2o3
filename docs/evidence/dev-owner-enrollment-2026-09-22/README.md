# Public Batch Enrollment Correspondence

Developer qualification of the journal, stable-reader and producer-reader
`enroll_allocations` adapters. Source: the signed commit in `SOURCE`.

The public methods and actual-type Verus harness execute the same forwarding
macros and unchanged enrollment, adaptive sorting and search bodies. Frozen
three-layer CPU adapters are authenticated against `ea7d4e0f2`; both paths use
the same algorithm. Scalar enrollment is outside this checkpoint.

## Recorded Checks

- Actual execution root: 270 verified, zero errors.
- Complete historical correspondence root: 829 verified, zero errors, twice.
- Nine scoped mutations reject omitted forwarding, producer-state corruption,
  skipped replay, commit or output clearing, and omitted actual/historical calls.
- Runtime-model CPU suite: 942 unit tests and 27 doctests passed; 18 ignored.
- Formatting, all-target Clippy with warnings denied, and release build passed.

`records/` contains byte-exact stdout/stderr and terminal process-group receipts.
The runner authenticates each immutable proof stage against the initial signed
source inventory before applying a declared mutation, brackets source hashes and
the pinned Verus distribution, and removes temporary proof stages. CPU commands
run in the recorded worktree with before/after source checks. The auditor checks
exact test totals in the captured logs, not just command exit status.

## Scope

Raw forwarding needs no reader, producer, issuance or custody precondition.
Rejections preserve complete normal owner values and caller output. Success
preserves outer reader/producer fields and untouched journal vectors exactly;
empty actual success preserves the whole owner value. The independently executed
historical journal agrees in result, ordered contents and output. Preservation
of producer custody and retained statuses is conditional on the initial invariant.

Synthetic witnesses exercise reordered slot associations, late rollback,
conditional invariant preservation, and malformed irrelevant storage. CPU frozen
comparisons cover all four producer statuses, stable leases, stale references
after fresh-ID slot reuse, error precedence, and physical buffer identities.
Replay counters measure only the allocation-arena scan, not sorting or total cost.

This does not refine fallible Rust construction, physical allocation, unwind,
native admission, backend completion, machine code, or HIP/HSA performance.
Captured negative diagnostics are shape-checked developer records, not a
precommitted exact-diagnostic qualification. The auditor checks integrity and
Git-object provenance; it does not rerun Verus or independently attest execution.

## Replay

From the repository root:

```sh
python3 docs/evidence/dev-owner-enrollment-2026-09-22/audit.py --repo . --selftest
```

For a fresh campaign, check out the signed `SOURCE` and invoke
`crates/fe2o3-runtime-model/verus/check-owner-enrollment.py` with the pinned Verus
binary, a new output directory and a separately owned Cargo target directory.
Use `--help` for the required arguments. The recorded campaign used no SSH/GPU
resources. Only its owned local scratch was removed after successful audit.
