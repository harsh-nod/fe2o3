# N5 Native Adoption Development Evidence

This packet records backend-owned nonpublishing DATA adoption, four native
binding routes, exact lane leasing, rooted cancellation and private Context/async
integration above `722cdcee9a617fe4a135cf3707e714c5d9cba79c`.

It is development evidence, not N5/R126 acceptance, production Worker V3
authentication, formal correspondence, hardware fault qualification, aggregate
memory closure, a benchmark or HIP/HSA parity. R125 remains the accepted Native
CPU/test checkpoint; A1/A2 and #182 remain open.

## Reproduction

`source.patch` is the complete 14-file Rust delta from `source-base.txt`.
`source-files.sha256` pins those exact files. Each recorded command has its
argument vector, UTC start/finish, complete combined output and exit status in
`raw/`. `record.sh` refuses to overwrite an existing command record.
`audit.sh` checks the source delta and final result assertions against the current
checkout; it requires this exact source cohort. `SHA256SUMS` closes the archive.

The preliminary `gnu-runtime` and `musl-runtime` records each passed 861 tests
with 13 ignored before the final two guard regressions. `clippy` exited 101 on
unnecessary unwrap/lazy evaluation diagnostics. The corrected `clippy-final`
record must exit zero. Preliminary results are not substituted for the final
source's `gnu-final` and `musl-final` results.

Final validation checks both complete runtime suites, strict
all-feature/all-target Clippy, formatting, 33 runtime doctests, no-default build
and the unsafe-source policy (five passes, one explicit maintenance ignore).
The runtime suites include eleven new CPU tests and two new ignored native
probes. No lower KFD source changed and its full test suites were not rerun;
this packet does not count prior lower results as new passes.

Final results: GNU and musl each passed **863 runtime tests**, with 13 ignored.
Clippy, final formatting, 33 doctests, no-default compilation and the unsafe
inventory all passed. Both new native probes are among the ignored tests, not
passes. The binary hashes and full test rosters are recorded separately for
each target. `audit.log` records a successful source/command-result audit.

## Scope Of Tests

The CPU tests cover backend shell/phase exclusion, exact suffix and lower-handoff
retention on first/middle/last release error and panic, callback destructor panic,
complete retirement counts, same-stream guards without generated buffer
arguments, terminal diagnostics, first-panic transport, and fail-closed Drop in
an isolated child process. Context tests cover authenticated empty-prefix Stop,
foreign/stale holds and shell credit disposal before hold release. Existing
source-identity, currentness and async lifecycle regressions also run.

These tests use metadata-only or generic ownership fixtures where a GPU owner
cannot be constructed locally. They do not prove native initialization/cleanup
fault prefixes or a single end-to-end concrete generated async operation.

Two compiled, ignored probes use the real repository vecadd qualification fixture,
checked KFD device and loader/ABI, then exercise cold/bootstrap primary, AUX,
rebound and three pristine aborts without submitting. They bypass only carrier
registration, use no synthetic Worker authority, and qualify backend mechanics
only when actually executed. They are **not executed in this packet**.

## Shared Host

`native-availability` and `native-processes` show all eight MI300X GPUs occupied.
GPU 0's zero utilization did not make it available: it retained a mapped workload
with about 91.5 GB VRAM. GPUs 1-7 were busy. `native-space` records available
filesystem space; availability was observation, not reservation.

No remote scratch, GPU work, deletion or process signal was performed. Native
route/failure qualification, Context/carrier/async qualification, profiler event
pairing, formal correspondence and matched performance remain outstanding.

Two read-only source reviewers found no remaining blocker in the reviewed scope.
They verified original packet custody, the four route branches, queue creation
event placement, reciprocal persistent exclusion, panic handling, private async
hooks, exact staged source hashes and the source patch. These scoped reviews do
not replace the missing hardware or end-to-end qualification.
