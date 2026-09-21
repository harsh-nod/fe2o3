# Complete Logical Enrollment Qualification

Signed source: `97ea2f97a2473bb8ebc1944a8efff18866d3b72a`.

| Check | Result |
| --- | --- |
| Whole-crate positive before and after | 263 verified, 0 errors each |
| Executable-body controls | 21 cases, each 262 verified and exactly 1 intended postcondition failure |
| Local checker adverse tests | 43 diagnostic/result cases and 15 source cases rejected |
| Portable evidence adverse tests | 26 cases rejected |
| Source and process custody | Clean signed HEAD brackets; 37 unchanged input identities; 34 owned process groups absent |

The 263 obligations comprise 201 inherited and 62 local obligations. Counts
overlap importing campaigns and must not be summed. The complete pinned Verus
distribution passed closure checks before and after: 190 files, 129019839 bytes.
The solver used default resource limits, four threads and a 180-second deadline
per whole-crate case. Partial runs, compiler errors, timeouts, unrelated failures
and expanded or misplaced diagnostic spans are not accepted controls.

The executable logical model now connects exact admission and error precedence,
full-key replay, free-slot validation, proved heapsort and binary searches,
unchanged-on-error rollback, canonical output restoration and commit. Its wrapper
preserves producer/stable-reader storage, all four legitimate producer statuses,
allocation and pending-chain custody, and the same writer issuance history.
It does not assume globally idle readers or writers. A constructor-based witness
executes nonempty enrollment, replay rejection, a lower unused allocation key and
writer registration; it does not reach Pending.

Production still uses standard-library sorting/search. This packet does not
establish their correspondence, integrate the new helpers into Rust, bind logical
storage to physical Vec capacity, prove panic/unwind behavior, or validate native
completion. No Cargo test, GPU operation, MI300X session, HIP/HSA comparison or
performance benchmark ran for this proof-only change. It does not close a native
runtime milestone or establish runtime parity. The standalone enrollment checker
is not part of the shared proof runner; that complete runner was not rerun here.
See the [current scope](../../runtime-context-version-enrollment-v1.md).

## Offline Audit

The archive retains unmodified source candidates, tool commands, raw streams,
process receipts, input identities and the original qualification driver.
`case-summary.json` is derived from those receipts. Receipt timestamps use the
realtime clock and are not performance measurements.

From a checkout containing the signed source commit:

```sh
python3 -I -B docs/evidence/dev-enrollment-execution-verus-2026-09-21/archive.py \
  --repo . --archive docs/evidence/dev-enrollment-execution-verus-2026-09-21
python3 -I -B docs/evidence/dev-enrollment-execution-verus-2026-09-21/archive-selftest.py \
  --repo . --archive docs/evidence/dev-enrollment-execution-verus-2026-09-21
```

The validator reconstructs pinned proof/checker bytes from the fixed Git commit,
regenerates every mutation and audited include, and rechecks exact diagnostics,
commands, source/receipt rosters, process cleanup and scope. It also validates
the derived case summary and complete packet manifest. It does not require the
original scratch paths or Verus installation, and it does not rerun Verus.
Receipt helpers are hash-pinned from the preceding issuance archive in the
same source commit. Authenticity depends on the signed Git publication.

`SHA256SUMS` covers all 435 other packet files. The preliminary focused solver
probes are not part of this qualification. To rerun the standalone campaign,
use the pinned Verus installation and a new, nonexistent output directory:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-journal-enrollment.py \
  --verus "$VERUS" --timeout 180 --output "$NEW_OUTPUT_DIRECTORY"
```
