# R126 Typed SDMA Allocation Disposition: Development Receipt

This packet extends `204aa3c089d6237e27b0c420e641d98abf750e8a`.
R125 remains the accepted CPU/test checkpoint. R126, A1/A2, issue #182,
formal implementation correspondence and HIP/HSA parity remain incomplete.

`source-files.sha256` identifies all eleven changed Rust files. The complete
source/test delta is `source.patch`, SHA-256
`9a4557461a4431b76bd5bb819bcfc9134b7bf5f2484e4f84a03be384c4074d69`.
Independent read-only review checked the manifest against disk and index and
the patch against the complete staged source delta. Source and oracle review
found no remaining blocker; those reviews are not formal proofs or test runs.

## Implementation

The retained lower allocation driver returns its original typed error with a
separate disposition. Retry authority requires exact host-visible/device
backing-credit `Capacity`, successful model retake, no retained allocation or
other SDMA mutation root, an owned model foundation and healthy memory/session
state. Merely observing no terminal flag, retained allocation or quarantine is
not sufficient. Host bookkeeping can still advance, including model revisions
and the pool activity latch. All other classified errors conservatively supply
no retry authority. Legacy APIs preserve their prior errors and state behavior
by erasing only this new disposition. Pool hits retain the existing algorithm.

Runtime samples primary/SDMA readiness before queue creation. A proven capacity
failure on a warm route is `Rejected(Capacity)`; after cold queue creation it is
`Quiescent(Capacity)`. Configured Context admission refunds only the warm case.
Cold quiescent rejection retains a quarantined credit without a public handle;
the backend remains healthy but that Context cannot fully clean up the credit.
This existing conservative facade contract is not silently changed or described
as HIP/HSA memory parity.

Staging allocations share the helper. Public read/write and partial-copy paths
retain their prior-effect upgrades. Recovered promotion and successful hidden
cleanup after failed zero initialization now return `Quiescent`, not `Rejected`,
because an allocation was already created. Terminal cleanup takes precedence.
Driver selection, allocation and typed diagnostic formatting are unwind-guarded;
the first panic payload and neighboring owners survive. There is no new unsafe
code, message-based error matching or separate allocation algorithm.

## Test Scope

Six added KFD tests exercise exact typed capacity errors, display/source and
legacy compatibility, original backing/accounting snapshots, successful retry
and cleanup, retake/native/validation failures and final settlement-witness
denial. The latter denies only a fixture witness while real memory admission
and model retake execute; it is not a concrete native-adapter predicate proof.
Public pooled host/device hit and miss tests preserve owner identity, generation,
accounting and legacy errors in explicitly engine-less fixtures.

Eight added runtime tests cover warm/cold provenance, neighboring allocations,
terminal capacity/protocol errors, actual scripted allocation panic, original
boxed diagnostic-panic identity, missing-driver selection, configured and
unconfigured Context credits, hidden cleanup and first/second copy chunks.
Hidden cleanup asserts exact errors/panic and indexed-versus-driver owner
location. Chunk tests compare the full expected owner/account/shadow snapshot,
allowing only the expected copied prefix to change. Scripted provenance is not
native queue-creation evidence. Terminal fixture disarming is not native cleanup
or credit-refund evidence.

The preliminary broader runtime SDMA run passed 119 tests and failed two older
`Rejected` expectations after promotion/hidden cleanup. Both now require
`Quiescent`; the original ownership, cleanup and error-kind assertions remain.
The raw failed run is retained separately from final results.

## Final CPU Results

| Target | All-Feature Library Suite | Passed | Failed | Ignored | Libtest Duration |
| --- | --- | ---: | ---: | ---: | ---: |
| GNU | KFD | 1,372 | 0 | 0 | 1,264.80 s |
| GNU | Runtime | 824 | 0 | 6 | 24.87 s |
| musl | KFD | 1,372 | 0 | 0 | 1,889.67 s |
| musl | Runtime | 824 | 0 | 6 | 26.60 s |

Both builds, strict all-feature/all-target Clippy, formatting, runtime
no-default-feature compilation and before/after source checks return zero.
Unsafe-source policy passes five tests with its explicit maintenance test
ignored. The six ignored runtime tests require isolated native hardware.
Durations are execution provenance, not performance benchmarks.

The runner records command arguments, UTC start/finish and process exit status.
The initial/strengthened builds, focused tests, early full GNU runtime run,
preliminary Clippy and native availability checks are separate operator-run
logs. The early full runtime run also passes 824/0/6; its stdout alone does not
authenticate the executable identity. The table above uses the collected final
run, not that preliminary result.
No earlier packet's full-suite result is substituted for this source.

Raw log endings, command spacing and unified-diff context are preserved rather
than rewritten for whitespace checks. `archive-checks.json` separates the
operator-run source/documentation check from the unfiltered raw archive check.
`SHA256SUMS` covers the archive's other files.

## Qualification Boundary

The shared MI300X availability checks show allocated VRAM on all eight GPUs and
active compute on several. No GPU job, remote upload or remote scratch directory
was created for this packet. Previous packets' hardware results are not reused
as current-source evidence. Native cold/warm capacity behavior, public fault
retention, additional profiles, pending-compute allocation, formal correspondence,
aggregate-memory qualification and matched HIP/HSA performance remain open.

`verify-allocation.sh CHECKOUT OUTPUT_DIRECTORY` runs the CPU development
campaign with locked offline dependencies, `jq`, GNU `prlimit` and the installed
GNU/musl Rust targets. The output directory must already exist. Its adjacent
source manifest is checked before and after execution; build logs identify each
test executable and separate manifests record their hashes. This is not the
closed full-workspace qualification runner or a benchmark. No test binary is
included in the archive.
