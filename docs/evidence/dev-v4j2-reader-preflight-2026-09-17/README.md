# V4-J2 Reader Preflight Qualification

All 20 qualification command records closed successfully between
2026-09-17 17:50:13 and 18:42:57 UTC. `SHA256SUMS` binds the final archive contents.
These are fresh results; the preliminary campaign is not substituted for them.
This is a completed development proof packet, not runtime milestone acceptance.

## Source And Scope

Base: `ed5b5d64bf95116c21e9bf350c30132ff2bbb524`. Twelve source/documentation
files are captured by `source-files.list`, `source-files.sha256` and `source.patch`.
Source manifest SHA:
`6b2df6120de4ebc9f6a4d40cc9e6225987944cdb0c379b6990c02664ecb5bb46`.
Patch SHA:
`e1444de9a657da20106e98d22b73fb0f2d49c7b5ed38bb9b5bae116b512cc311`.

The [development contract](../../runtime-context-read-preflight-v1.md) describes
the executable proof and production correspondence review. The proof imports
unchanged, separately pinned V4-J1 definitions. Of its 103 whole-crate obligations,
69 are inherited and 34 are new reader obligations; inherited work is not counted
as new verification.

Proved scope: constructor contents under successful storage; exact allocation
and lease lookup; ordered read, capacity, acquisition/release preflight and unread
guard decisions; complete proof-side contents/output framing during preflight.
Weak representation premises preserve malformed-state rejection cases.

Not proved: acquisition/release commits, healthy-invariant preservation, base
membership/settlement reachability, production Rust refinement, physical storage,
panics/unwind, Context ordinary/generated composition, initialized inputs,
authenticated native quiescence, machine-code execution or performance.
Quiescence evidence and observed free capacity remain explicit external premises.
Accepted Native R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3 checkpoints
are unchanged; A1/A2 and #182 are not closed by this packet.

## Qualification Results

The exact commands, UTC start/finish times, output and exit status are in `raw`.
The sequential `qualify.sh` completed:

- Fresh runner structural self-test and authenticated inventory of all 686
  unchanged legacy negative sources, including six new reader-wiring fixtures.
- Complete GNU and scoped musl runtime, host and model unit suites, with matching
  2,144-entry rosters: 2,121 passed and 23 ignored each. Runtime contributes
  1,057/17, host 271/4 and model 793/2; failed/measured/filtered counts are zero.
  All 2,139 baseline entries are preserved, with exactly five added reader tests.
- Strict all-target/all-feature Clippy, formatting and no-default checks;
  unsafe-source policy (5 passed, 1 ignored) and all 87 doctests.
- Reader self-tests: 21 reversible body mutations, 38 rejected adverse diagnostic
  or source cases, a timestamp-valid stale-bytecode regression and inherited
  owned-process cleanup tests.
- A standalone reader campaign with two 103/0 positive runs and 21 intended
  102/1 negative runs, exact postcondition/exit spans, no compiler/VIR/unrelated
  failures, authenticated source/tool closures and absent owned process groups.
- The complete integrated global Verus gate, including the unchanged J1 campaign,
  the reader campaign and all 686 legacy negative files. Its exact final transcript
  SHA is `78ca7b3c6d910dc47969aaddd8bd57253c643b9635f20ed395562c2449925ca3`.
- Source hashes before and after the complete qualification. Six test-produced
  executables are first hashed after GNU/musl tests, then checked unchanged after
  the remaining gates; this is not a pre-test binary identity claim.

Rust witnesses strengthen three existing matrices to exact error enums and add
five constructor/precedence/group/slot/headroom tests. Existing ABA, overlapping
ranges, version-gap and 4,000-step partition traces remain executable evidence,
not proofs of commits. Malformed base pending backlinks are covered by the Verus
decision model but are not constructed through the safe Rust API in these tests.

## Separate Preliminary Cohort

The [preliminary archive](../dev-v4j2-reader-preflight-preliminary-2026-09-17/README.md)
is immutable and separately sealed. It passed CPU checks and the full reader
campaign, then failed the global runner's complete-source structural audit.
The failure was not waived: this successor updates the canonical tool bindings,
campaign/digest/order checks, six adverse fixtures and authenticated expectations,
and reruns qualification from a fresh twelve-file capture.

Preliminary manifest SHA:
`d486bf715b0f72fed6a5d632d2a550e9eaea0ea3d69a0985bdc3a43d844675a8`.
Its 270 entries and 18 records are closed; only `global-verus` exited 1.
Clarification of its sealed wording: the preliminary binary identity interval
also starts **after** CPU tests, not before qualification. The current receipt
does not reinterpret or overwrite that historical archive.

Earlier unfrozen exploration exposed a Python cached-bytecode identity gap and
multi-exit diagnostics from guard-deletion mutations. The checker now compiles
the exact verified helper bytes, and mutations corrupt single decision branches
without relaxing the strict intended-error parser. These development failures
are not counted as accepted qualification negatives.

## Hardware Boundary

Read-only MI300X observations at 17:12-17:13, 17:32 and 17:54-17:55 UTC on
2026-09-17 found every GPU occupied by mapped processes and substantial VRAM.
Zero utilization on GPU 0, and later GPU 7, did not establish availability.
These earlier observations were not treated as a reservation requirement.
GPUs 1-7 became free at 18:13 UTC. A separate
[directional-copy diagnostic](../dev-directional-copy-mi300x-2026-09-17/README.md)
then used GPU 1 at the committed runtime base; its build, measurements and cleanup
are not part of this proof qualification. There is no native reader-lease or
matched HIP/HSA parity result here. The user's authorization to use genuinely
free GPUs was sufficient; no exclusive reservation was required.
