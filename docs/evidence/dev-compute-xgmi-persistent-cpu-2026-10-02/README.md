# Persistent Compute-XGMI Runtime CPU Checks

## Source and Scope

Source base: `f97257ed84c8a39e82a27955cd3f146df0d16781`.
The raw packet includes the exact source/guard patch, its SHA-256, changed-path
hashes, compiler versions, command scripts, complete outputs and exit records.
Documentation and evidence are outside the source patch. The base checkpoint
was pushed to both repositories before this increment.

The implementation preserves the original persistent SDMA owners, identities,
generations and physical pool extents across one complete logical XGMI copy.
Both VM models are restored before either persistent output is published.
The runtime uses its existing cooperative-copy submission, dependency, event
and allocation-retain indexes. Only the opt-in R57 peer qualification profile
admits native routes. Ordinary allocation defaults remain private.

Native selection requires initialized PUBLIC storage, complete equal logical
extents within one packet, normalizable persistent backing and quiescent
endpoints. Other copy shapes retain host staging. A selected native transfer
reserves no host payload staging and cannot fall back after native effects.
Success follows native completion, peer queue retirement and both allocation
restorations. Uncertain failures retain ownership and poison both children.

The new two-GPU witness checks four exact launches, replacement of a distinct
destination sentinel, unchanged source, native completion count, 13 full-buffer
readbacks and explicit cleanup. Its two CPU tests exercise argument parsing
and sentinel discrimination, not GPU execution.

## Evidence Boundaries

The library builds use test optimization level 1, with debug assertions and
overflow checks enabled, one build job and serial tests. These are not
unoptimized-debug results. The exact clean environments and locked/offline
commands are retained.

The runtime tests use a scripted transport inside the actual cooperative
submission ledger. They cover eligibility/fallback, dependency and observation
behavior, owner identity, destination metadata, release discipline and error
or panic at create/copy/retire/restore boundaries. Scripted success deliberately
does not increment the native completion counter.

The persistent KFD tests compose production detach/restore with the paired
model-loan sequencer. Their callback does not execute the complete native
map/packet/completion path. Existing lower mapping and SDMA tests remain
separate. Source-CI identity updates do not prove this new native route.

Initial rejected checks are retained, not replaced by later outcomes:

- Runtime check: a borrow held across failure handling; corrected before the
  combined test build.
- Strict Clippy: an infallible match; corrected before final checks.
- Formatting: KFD module ordering and one line wrap; corrected mechanically.
- KFD `initialized`: 62 passed and one stale source-guard failure. The guard
  banned PUBLIC anywhere in a shared file, including its new explicit PUBLIC
  constructor. The correction keeps whole-module no-initialization/custody
  checks, binds private defaults and request wiring explicitly, and scopes
  PUBLIC bans to ordinary paths. A constructed private/public/private sequence
  and uninitialized-content assertions strengthen the policy checks.

The full runtime baseline was independently rerun before the new build:
1,929 passed, three failed and 32 ignored. The three existing telemetry tests
all fail at `authorized_execution.rs:1317` with `InspectSocket` `EPERM`.
No skip, weakened assertion or permission workaround was introduced.

## Final CPU Results

The corrected combined library test build exits zero in 3m51s. The final runtime
ELF is `55d1270bc198e67902a34d4fed47a79f13ef6c36753729e036553105a99c0658`.
Its focused `compute_xgmi` run passes all 12 tests, with no ignores or failures.
The unfiltered run records **1,941 passed, three failed and 32 ignored** in
97.91s, exit 101. The same three `InspectSocket` permission failures remain;
there are no additional failures. This is not a full-suite pass. All before
and after ELF hashes match; `runtime-tests-corrected/` retains the records.

Final KFD ELF:
`9bd96d9427dc92eb6fffb697e9f425602fa49f7d4f6aab1464cd780549361c9b`.
All five corrected focused commands exit zero with no failures or ignores:

| Filter | Passed | Filtered out | Seconds |
| --- | ---: | ---: | ---: |
| `initialized` | 63 | 1,825 | 126.08 |
| `compute_xgmi` | 25 | 1,863 | 0.05 |
| `model_pair_loan` | 7 | 1,881 | 0.00 |
| `public_sdma` | 6 | 1,882 | 2.46 |
| Ordinary private/Host allocation baseline | 1 | 1,887 | 1.73 |

These are overlapping filters, not an aggregate full-suite count. The repaired
guard, new mixed-profile case and all 13 initialized-storage conversion tests
pass. Complete rosters and matching before/after hashes are in `kfd-corrected/`.

The final example CPU run passes both tests, and formatting and whitespace
checks pass. The no-default library check exits zero with one dead-code warning:
the native route can be constructed only with `hardware-qualification` enabled.
That warning is retained in `verification-final/no-default.stderr`.

Strict combined all-feature library/test/example Clippy with `-D warnings`
exits zero in 1m20s. The normal, runnable smoke example builds with
`--profile test` in 9.41s, exit zero; it was not executed. Its SHA-256 is
`99a28ea37d60f7dfa2c3f93b9cab0c6c612f76f0e29cb072bcf53178c002e844`.
Both unit-test ELF hashes remain unchanged after this tooling.

## Source Controls

Both source-control attempts pass all 32 existing workflow commands. The final
`attempt-02-after` takes 91.65 seconds, with 7,694 inputs unchanged throughout,
no timeouts and all owned process groups absent. It runs no build, GPU command
or solver. The final metadata audit permits exactly 19 SHA literals and seven
source-inventory counts across 12 scripts; all 76 associated executable proof
files remain unchanged.

The second attempt incorporates only four declared KFD changes after the first:
the two test-only policy corrections and two formatting changes. Three hash
values are refreshed within the original whitelist. Historical audit records
remain intact. This is source-binding maintenance, not new formal acceptance.

## Remaining Work

This qualification-only route executes synchronously within flush/drain, with
a 30-second native completion wait. A drain deadline is checked between steps,
not inside that wait. Poll and wait remain observers. Asynchronous peer
progress, persistent mappings, multi-packet copies and broader workloads are
not accepted by this increment.

The complete native fault composition, unrelated dirty-cache reconciliation,
full loaded-kernel consumer interaction and drain-deadline behavior need
additional coverage. The separate proposed composed CPU test plan is included
as unimplemented planning, not test evidence. The broad KFD suite remains
incomplete; focused passes cannot substitute for it.

The latest MI300X attempts fail at SSH hostname resolution, before any remote
command. No GPU pair is currently admitted, no GPU workload ran and no remote
scratch was created. Hardware correctness, physical concurrency, scaling,
matched HIP/HSA performance and machine-code refinement remain unverified.
No milestone exit or general parity claim follows.

## Raw Packet

`raw.tar.gz` retains the initial and corrected checks separately, source-CI
receipts/audits, the unchanged-baseline telemetry reproduction and read-only
connectivity records. `SHA256SUMS` identifies the archive. Executables and the
Cargo cache are not bundled; replay needs the recorded toolchain and locked
dependencies. All source is recoverable from the base plus `source.patch`.
