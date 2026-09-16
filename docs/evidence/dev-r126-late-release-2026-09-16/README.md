# R126 Late Release: Development Receipt

This packet closes the remaining ordinary-primary constructed-parent CPU joins
for late resource/signal failures and pristine-abort detached-ledger admission.
It does not accept R126, close A1/A2 or #182, establish native/formal
correspondence, or demonstrate HIP/HSA parity or performance gains. R125 remains
the accepted local CPU/test checkpoint.

## Source And Scope

Base: `14887d5ad9b3e88ae19dcad8751237b68cfe1b47`.
`source.patch` contains the complete KFD source delta, including new files, with
zero context; apply using `git apply --unidiff-zero`. Its SHA-256 is
`32c5d50c8756cfe1311f6a8e8bba6e56c48b5ad08a11261d23960d7c11cb8e36`.
`SHA256SUMS` binds the patch, toolchain records, executable hashes and raw logs.
No binary is checked in. Private frozen copies prevent concurrent builds from
replacing libtest executables whose tests spawn `current_exe()`.

Production's borrowed detached-dispatch admission predicate is extracted without
changing its logic and shared with the genuine constructed-parent fixture. The
historical clear-unpublished-state branch is not tightened in this packet.
The remaining source changes are fixtures, failure injection and test oracles.

The new/expanded constructed matrices cover errors and panics separately:

- All 24 queue-resource currentness boundaries.
- All eight model stages for each of four queue resources.
- All six signal currentness boundaries and eight signal model stages.
- Exact native-call, retained-owner, model and account prefixes; untouched
  suffixes and backing bytes; original identities/layouts; terminal parent,
  retained gate and inert retry.

The signal oracle separates successful native disposal from account settlement
and model commitment. Closing-currentness failure retains a disposed receipt
without refunding. Release-commit failure refunds native accounting while the
model remains unmapped. Signal `UnmapPreflight`/`UnmapEvidence` errors preserve
the lower phase; panics and all currentness failures quarantine it.

The former post-abort fixture is replaced: the actual continuation and full
returned identity ledger are installed on the parent after the abort loan is
reclaimed. Every original buffer runs the shared detached-data release driver
with one real loan/retake. Borrowed teardown admission rejects every partial
ledger and malformed final count, identity, recycled generation or insertion
coordinate without effects. Final release refunds all backing and preserves
the complete settled ledger through rejected retry.

These are genuine allocated/mapped CPU fixture owners and production drivers,
not public Linux facade or hardware fault qualification. No revision-exhaustion
injection is claimed after permanent restoration revokes the certificate.

## Validation

Both target builds used `cargo test --locked --offline -p fe2o3-kfd
-p fe2o3-runtime --all-features --lib --no-run`; the musl build added
`--target x86_64-unknown-linux-musl`. Frozen KFD binaries ran:

```sh
<kfd-binary> integration_tests::release_cases:: shared_memory::tests \
  sdma_cleanup primary_release::tests --test-threads=16
<runtime-binary> --test-threads=16
```

| Check | GNU | Musl |
| --- | --- | --- |
| Selected KFD tests | 308 passed, 1,000 filtered | 308 passed, 1,000 filtered |
| Constructed release cohort, included above | 30 passed | 30 passed |
| Runtime library | 745 passed, 1 hardware test ignored | 745 passed, 1 hardware test ignored |

KFD runs took 153.40s/150.45s; runtime runs took 27.51s/32.10s. These are test
durations, not runtime performance measurements. Both builds and runs exited 0.
Source was unchanged during passing validation and its patch hash was rechecked.

Strict Clippy passed for both crates with `--all-features --all-targets --
-D warnings`. `cargo fmt --all -- --check` passed. The unchanged unsafe-source
policy passed five tests; its explicit inventory-refresh test remained ignored.
These are focused KFD and full runtime library regressions, not fresh full KFD
or workspace qualification, compiled-negative campaigns or formal proofs.

Two read-only reviewers checked the shared predicate, actual abort/ledger join,
model/native/refund formulas and test assertions. Reported fixture/oracle gaps
were corrected before final validation; both rechecks found no remaining
blocker within their scope. No SSH commands, remote artifacts, GPU execution,
native fault injection or benchmarks were performed for this packet.
