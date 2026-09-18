# V4-J4 Reader Invariant Qualification

Qualification completed with all twenty-two command records closed at exit 0,
including the complete integrated gate and final source/binary identity checks.
The recorded interval begins on 2026-09-17 at 21:23:57 UTC and ends on
2026-09-18 at 00:47:14 UTC; the timestamp caveat below applies to that interval.
`SHA256SUMS` seals the archive.

## Source And Scope

Base: `f815d1dc7406851e39e77f488b6bc4b4b3f94ae2`. Eleven source/documentation
files are captured by `source-files.list`, `source-files.sha256` and `source.patch`.
Source manifest SHA:
`35aadbd8b642c57095f0fdaf9437bb888c218ba0b38c68943ce3262f4855f810`.
Patch SHA:
`5e74f1b3a7193ad6d076ac77c9784cd86c7f1f36b90251925f6a4b7b26ebd0d3`.

The [development contract](../../runtime-context-read-invariant-v1.md) describes
the concrete free/occupied partition, exact reader multiplicities, valid live
references and requests, and distinct reader incarnations. The proof establishes
constructor/acquire/release preservation, derives the previous selected-free
uniqueness premise, and proves exact count, lookup and unread-exclusion
consequences. Register/abort wrappers discharge a field-level reader frame.

The proof has 155 whole-crate obligations: 127 inherited through unchanged
pinned J3/J2/J1 sources, plus 28 new. Its history theorem is bound to exact
modeled acquire/release/register/abort transitions, including actual successful
acquisition outputs, rather than an independent numeric watermark trace.
The reader-valid initial state has watermark 1 and may already be enrolled.

The nonempty formal witness explicitly installs a fixture allocation, acquires
overlapping readers, releases in reverse order and reuses a slot with a fresh
incarnation while rejecting the old reference. This is a verified executable
call chain, not a separately assembled witness-specific ghost trace or a
production enrollment proof.

No runtime production behavior changes. Full base writer/member invariants,
general enrollment/Begin/settlement/retirement/Unknown composition, production
Rust/native refinement, physical storage/capacity, unwind and authenticated
quiescence remain separate. Accepted Native R125 CPU/test, Admission R118B C1-C3
and Resources R116/V3 checkpoints are unchanged. A1/A2, #182 and HIP/HSA parity
remain incomplete.

The next lifecycle proof must derive reader framing from the executable
enrollment and unread-retirement validations. Allocation-key freshness is
a separate premise from the reader-incarnation theorem: the standalone model
tracks live allocation keys, not historical enrollment identities. Its batch
API documents caller freshness, but the single and reader-wrapper APIs need
that premise stated explicitly. A read-only review found no healthy production
Context reissue path: ordinary and generated allocations share a nonwrapping
issuer, and retirement does not rewind it. J4 does not mechanically prove that
production correspondence. Generated-batch retirement followed by fresh
allocation on a reused slot remains a targeted regression-test follow-up.

## Qualification

`qualify.sh` records exact commands, UTC timestamps, full output and status.
The standalone invariant campaign adds one test-only executable subject whose
only postcondition is the invariant. It requires two exact 156/0 positives and
sixteen 155/1 negatives at the exact intended postcondition and in-body exit
spans. These are invariant-sensitivity tests, not production-body mutations.
The existing J3 reader-body campaign remains a separate integrated gate.
Compiler/VIR, timeout, extra and unrelated errors cannot qualify as negatives.

GNU and scoped musl each passed 2,124 unit tests, with 23 ignored and no
failed/measured/filtered tests: runtime 1,057/17, host 271/4, model 796/2.
Their exact 2,147-entry rosters match and retain all 2,146 baseline entries,
adding only the whole-reader lifecycle test. The standalone campaign passed
both positives and all sixteen intended negatives. An independent read-only
audit checked every diagnostic, all 20 closed owned-process records, 25 input
identities, generated source copies and the 190-file Verus release closure.

Strict all-target/all-feature Clippy, Rust formatting, Python lint/format,
no-default builds, unsafe-source policy (5 passed, 1 ignored) and all 87
doctests passed. The scoped musl run disables legacy HIP via
`FE2O3_HIP_SYS_DISABLE=1`; it does not qualify legacy HIP execution.

The complete integrated gate reran the unchanged J1/J2/J3 executable mutation
campaigns and the new J4 invariant campaign, then passed every existing positive
proof and all 686 legacy negative cases. Runner structural self-tests and the
authenticated legacy inventory also passed, including the six new invariant
campaign-wiring negatives. Inherited adverse-diagnostic/source tests, owned
process cleanup checks, reversible mutations and recursive include auditing
remain active. Final source, checker and Verus release closure authentication
passed (190 release files, 129,019,839 bytes). The final success transcript
matches pinned SHA
`0de077445b5d4f7377251ced55b8143e3367372edf2635883a7f9543ec98d3e8`.

The six test-produced executables are first hashed after both Rust test runs;
their final identity check passed and covers the later interval, not a pre-test
binary identity claim. All eleven captured source hashes matched before and
after qualification. The runner removed its owned temporary directory.

The Rust whole-state inspector independently recomputes the reader partition,
counts, identities, current versions and incarnation uniqueness. The new
lifecycle test retains readers on unrelated allocations during enrollment,
writer registration/abort, successful and no-effect settlement, Unknown disposal,
retirement and fresh-key slot reuse. It checks exclusion, partial release,
actual LIFO reader-slot reuse and unchanged reader-storage addresses/capacities.
The existing 4,000-step multi-consumer trace now checks the same invariant.
These are executable witnesses, not mechanical Rust-refinement theorems.

The seal checks the exact source base/manifest/patch, complete changed-file
roster outside both archives, staged/disk agreement, all twenty-two ordered
records, identical target rosters, binary identity and final pinned transcript.
It additionally authenticates the separately preserved stopped attempt below.

### Timestamp Caveat

`invariant-campaign/allocation_identity/solver/record.json` records realtime
timestamps 1789680827294993248 and 1789685657952386925 nanoseconds, an envelope
of approximately 4,830.657 seconds despite a configured 180-second inner
timeout. The record has normal exit 1, no recorded timeout exception and
`group_absent=true`; the exact 155/1 proof result and intended diagnostic
passed independent inspection. These records do not establish whether a
clock adjustment, suspension or another cause explains the envelope. Raw
timestamps are preserved unchanged. No elapsed solver-runtime or timeout
wall-clock bound is claimed. This does not affect the source-bound proof
result, and the integrated campaign independently reran the same case.

`integrated-allocation-identity` retains the complete closed case from that
integrated rerun, copied after its success marker and checked byte-for-byte
against the runner's temporary directory. Original temporary execution paths
remain in its records. It reports the same intended 155/1 result, normal exit 1,
recorded group absence and a 73.510-second realtime envelope. This corroborates
the proof rejection; it does not explain the earlier timestamp gap or establish
a solver-performance result.

## Preserved Preliminary Attempt

The [preliminary archive](../dev-v4j4-reader-invariant-preliminary-2026-09-17/README.md)
is unsuccessful evidence, independently sealed with manifest SHA
`d2b55db01a1d3673ba817884c22a46ec35203884c753d8dde208ad6a58a75017`.
Seven commands passed before the first campaign stopped. Its positive passed
156/0; the first mutation produced 155/1 at the intended invariant postcondition,
but Verus located the implicit unit return at the function signature, outside
the strict parser's allowed body range. No negative was accepted.

The successor changes exactly two captured source files: an explicit `return ();`
in the generated test subject and the checker pin. Proof, parser, premises and
mutations are unchanged. An independent read-only review confirmed the exact
delta and all 75 preliminary manifest entries (76 files including the manifest).

## Hardware Boundary

No GPU workload, remote build directory or benchmark artifact is created by
this qualification. A read-only MI300X availability observation at
2026-09-17 21:25:38-21:26:01 UTC and a later observation at
23:14:57-23:15:27 UTC found mapped processes and substantial VRAM allocations
on all eight devices. A third observation on 2026-09-18 at 00:00:28-00:00:56 UTC
again found all eight occupied; the low-utilization GPU was not free.
No foreign workload or file was changed. Matched copy-performance testing
remains separate and awaits an actually available device.
