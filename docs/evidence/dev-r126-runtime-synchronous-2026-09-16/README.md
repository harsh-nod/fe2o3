# R126 Runtime Synchronous Copy And Readback: Development Receipt

This packet extends `9c9ff3290de9a48ec37b5d1d229ce7e526982995`.
R125 remains the accepted CPU/test checkpoint. R126, A1/A2, issue #182,
formal implementation correspondence and HIP/HSA parity remain incomplete.

The complete source/test delta is `source.patch`, SHA-256
`ad2359ae95da282e71de363dfdc160596f9fd530c5ca760e1776d6a609c1a03d`.
`source-files.sha256` records the eight changed Rust files. The final regression
runner checks these identities before and after execution. No lower KFD source
is changed in this packet, and its previous full-suite results are not counted
as fresh execution here.

## Runtime Ownership

The existing runtime terminal-custody slot now holds the synchronous operation's
host staging buffer before ready normalization. It subsequently retains the
device/host pair, original device-box shell, pending or completed submission,
or returned lower terminal custody. Driver selection precedes owner extraction.
The fused lower synchronous execution remains shared; no separate submit/wait
algorithm is introduced. Returned owners are installed before typed diagnostics.

Completed metadata is checked before frontier retirement. Native retirement is
a callback-free, queue-independent owner transition. Restoration requires the
original allocation slot to remain DeviceLocal/InFlight(Synchronous), and fills
the original box rather than allocating a replacement. Busy and prepublication
rejection recycle staging before returning the original error; restoration and
recycle failures take precedence. Terminal retries leave owners, credits and
recorded profiler events unchanged. Panics preserve the first payload.

Indexed host reads use the existing borrowed host-access guard. Transient
readback installs its staging owner before driver selection, host reading and
destination copying. Ordinary read errors still attempt cleanup, with cleanup
failure taking precedence. Read/copy panics retain staging without attempting
recycle. The recycler checks its required Buffer phase before taking custody.
Public readback still uses the existing allocated intermediate byte slice;
this packet does not change partial-copy visibility or claim zero-allocation
public downloads. No new unsafe block is introduced.
Inline returned-owner types can increase stack traffic; their native cost is
unmeasured.

## CPU Evidence

Eighteen new tests cover both copy directions with nonzero offsets and data,
exact owner identity and original Box reuse, healthy rejection/retry, timeout
and teardown, six corrupted completion-metadata cases, retirement failures and
panics, missing-driver and actual scripted slice-copy panics, and six changed
restoration slots. Occupied Host/Device replacements remain untouched. Typed
formatter-panic tests preserve the original boxed panic payload for timeout,
teardown and prepublication rejection.

Host-read coverage includes indexed certificates and unchanged destinations,
short/long read-length panics, transient read/recycle precedence, nonzero Device
bytes distinct from the host shadow, exact second-chunk visible prefixes,
Context credits and terminal retries, and core-disabled subprocess Drop guards.
The scripted driver retains its actual Pending/Completed owner across copy and
retirement panics; these are not fabricated owner IDs. The deliberate invalid
scripted-copy request bypasses public range admission to reach the lower panic
guard and is not evidence that the public API accepts out-of-bounds requests.

The synchronous core success test records zero heap allocations in both
directions and preserves the original device-box address. This is a counted
scripted-path assertion, not native performance evidence or an assertion about
ready normalization, public staging allocation or readback allocation.

| Target | Suite | Passed | Failed | Ignored | Libtest Duration |
| --- | --- | ---: | ---: | ---: | ---: |
| GNU | Runtime library | 816 | 0 | 6 | 30.86 s |
| musl | Runtime library | 816 | 0 | 6 | 40.41 s |

Durations are libtest-reported execution provenance, not benchmarks. Recorded
UTC start/finish intervals are 29 s (GNU) and 38 s (musl), shorter than these
libtest durations. The logs do not establish the cause, and this development
runner does not qualify clock agreement.

Strict all-feature/all-target runtime Clippy, formatting and no-default-feature
compilation pass. Unsafe inventory policy passes five tests with its explicit
maintenance test ignored. The preliminary
113-test focused run passes before the final lint-only annotations; final full
regressions, not that preliminary run, establish the final source results.
The script records commands, UTC start/finish times and process exit statuses;
build output identifies each executable and separate manifests record hashes.
This development runner is not the closed full-workspace qualification runner.
Clippy, formatting and availability observations were separate operator-run
commands, not collected by that runner. Their raw logs are retained. Commands:

```sh
bash verify-runtime.sh
cargo clippy --locked --offline -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo fmt --all -- --check
ssh -o BatchMode=yes mi300x 'rocm-smi --showuniqueid --showuse --showmemuse'
```

The development script embeds the original local checkout/output paths; relocate
those two variables before reproducing it elsewhere. No test executable is
included in this archive.

The runner's `whitespace.*` result precedes raw-evidence staging. The subsequent
source/documentation-only staged check also passes. The unfiltered initial
archive check returns 2 for preserved libtest blank endings, command trailing
spaces and unified-diff context lines; those raw bytes are intentionally not
rewritten. `archive-checks.json` records these separate operator-run checks.

Preliminary logs retain the initial test compile errors, a shared-ledger
snapshot oracle failure, and strict Clippy's large returned-owner diagnostics.
These were corrected, not accepted as passing evidence. The final annotations
are narrowly scoped to keeping returned owners inline across transfer, without
allocating a replacement on the failure path.

Bounded read-only reviews found no remaining production ownership blocker.
The final oracle review confirmed Async restoration rejection, exact occupied
replacement preservation, complete terminal-retry snapshots and teardown
formatter-panic coverage. These source reviews did not execute tests and are
not formal implementation proofs.

## Native Availability

Both MI300X availability observations show resident allocations on all devices
and active work on GPUs 1-7; the final observation also reports GPU 0 activity.
No hardware probe has run for this source,
and no remote executable or scratch directory has been created. Prior native
success for the lower synchronous packet does not qualify this runtime delta.
The shared machine was not reset, stopped or cleaned outside this task's scope.

## Remaining Scope

Fresh native public-workflow success and failure evidence remain open for this
runtime delta. Typed healthy capacity rejection, allocation during pending
compute and other SDMA profiles are next implementation work. Full R126 source
gates, compiled negatives and qualification are still required. Generated
DATA-ADOPT, ISSUE/COMPLETE, typed replies, Stop/drain/graphs, production Context
and resource proofs, multi-device behavior, device-language/collective refinement
and reproducible matched HIP/HSA performance remain part of the unchanged goal.
