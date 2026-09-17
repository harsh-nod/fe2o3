# V4-J3 Reader Commit Qualification

Qualification completed on 2026-09-17, from 19:19:04 to 20:10:16 UTC.
All twenty command records closed with exit 0, including the complete integrated
Verus gate and final source/binary identity checks. `SHA256SUMS` seals the archive.

## Source And Scope

Base: `04d9f3ca37cd17536901d4d7cab405bf06f54454`. Eleven source/documentation
files are captured by `source-files.list`, `source-files.sha256` and `source.patch`.
Source manifest SHA:
`6da0df0b2e545eeecb9a47d5088b6a2f0bf2cfbec9c1bd3c680b4a21041c375d`.
Patch SHA:
`2f072604b80147c8f82e689dac78cb593f915a3c87aaf1eb3a40e44375df1434`.

The [development contract](../../runtime-context-read-commit-v1.md) describes
the exact acquisition/release contents, rejected-call framing, ordered scan
bridges and installed-output theorem. Its whole-crate count is 127 obligations:
103 inherited and 24 new. The unchanged J2/J1 definitions are included/imported
as one exact pinned nominal type instance; inherited obligations are not counted
as new verification.

Acquisition requires selected-free uniqueness only on successful preflight.
Rejections retain the weak malformed-state boundary. Release-slot uniqueness is
derived from exact lookup and canonical order. This is conditional model contents
correctness, not full arena invariants, historical freshness, base membership or
settlement reachability, physical storage, unwind, Rust/native refinement,
authenticated quiescence, initialized inputs or machine-code execution.
No runtime production behavior is changed. Accepted Native R125 CPU/test,
Admission R118B C1-C3 and Resources R116/V3 checkpoints remain unchanged.
A1/A2 and #182 remain incomplete.

## Completed Checks

Exact commands, UTC timestamps, output and status are retained in `raw`.

- GNU and scoped musl each passed 2,123 unit tests, with 23 ignored and no
  failed/measured/filtered tests: runtime 1,057/17, host 271/4, model 795/2.
  The musl run used `FE2O3_HIP_SYS_DISABLE=1`; it excludes legacy HIP execution.
- Exact 2,146-entry rosters match across targets. All 2,144 baseline entries
  remain, with precisely the two added reader commit/epoch-boundary tests.
- Strict all-target/all-feature Clippy, formatting, no-default builds,
  unsafe-source policy (5 passed, 1 ignored) and all 87 doctests passed.
- Runner structural self-tests and the authenticated 686-file legacy-negative
  inventory passed, including six new commit-campaign wiring negatives.
- Commit self-tests reuse the strict parser's 38 rejected adverse diagnostics
  and sources, stale-bytecode defense and owned-process cleanup checks, then add
  ten include/recursive-dependency rejection cases and 15 reversible mutations.
- The standalone commit campaign passed two 127/0 positives and 15 intended
  126/1 negatives, with exact postcondition/exit spans and authenticated
  source/tool closure checks. Compiler/VIR, timeout or unrelated errors were not
  accepted as expected negatives.
- The integrated gate reran the unchanged J1 issuance and J2 preflight campaigns,
  passed the new J3 campaign, and passed all existing positive proofs and 686
  legacy negative cases. Final source, checker, Verus executable and release
  closure authentication passed (190 release files, 129,019,839 bytes).
- The final success transcript matches pinned SHA
  `174ca8101754f58d4a51a9f1b7fabb120fc7d42d2dec616c2e9cfed09baa8826`.

The new Rust witnesses compare exact lease, count, free-list and output contents
through a grouped batch, partial release and LIFO reuse, preserving retained
neighbors and reader storage identity. The other test acquires the last legal
incarnation interval, releases it, and verifies exhaustion remains closed.
Neither witness is a mechanical production-refinement theorem.

The six test-produced executables are first hashed after the GNU/musl tests.
Their final identity check passed and covers the later qualification interval,
not a pre-test binary identity interval. All eleven captured source hashes also
matched before and after qualification.

The seal additionally checks the exact base, manifest and patch hashes, ordered
source membership, complete non-archive changed-file roster, staged/disk agreement,
all twenty ordered records, matching test rosters and final transcript. It does
not promote these conditional model results into native runtime acceptance.

## Separate Hardware Diagnostic

The [single-packet engine diagnostic](../dev-single-packet-copy-engines-mi300x-2026-09-17/README.md)
used the committed runtime base on free MI300X GPU 1 while these checks ran.
It did not execute this proof or native reader-journal integration and does not
establish performance parity. Its eight runs, one preserved summarizer-harness
failure, corrected parser tests and removal of the owned 236 MiB remote directory
are sealed in their own archive.
