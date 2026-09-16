# R126 Directional Demotion: Development Receipt

This packet extends retained device ownership above
`a1cbebb3be1c850f2f5a136e63622c714caa1e32`. R125 remains the accepted CPU/test
checkpoint. R126, A1/A2, issue #182, formal correspondence and HIP/HSA parity
remain incomplete. Both full KFD/runtime regressions below completed successfully.

`source.patch` contains the complete KFD/runtime source/test and reviewed unsafe
inventory delta. Its SHA-256 is
`7630399b687f3bec36cd8a210d1abf44ce9dadaa383ecdc986330d3362dc223c`.
Documentation is separate. Source has stayed frozen during full regression and
native execution; executable identities are recorded in `binary-sha256.txt`.

## Ownership Contract

The lower driver preserves foreign-owner, enabled-state, exact-pair, generation
and Local-native admission order. It roots the original allocation before the
model loan, validates its borrowed mapping, and settles retake before existing
quiescent conversion. Successful demotion increments pool generation once;
backing identity/extent/accounting and outstanding-buffer debit are unchanged.
Healthy opening/validation rejection returns retryable custody. Retake error
outranks ordinary validation error; the original panic survives retake/poison
panic and leaves custody in the terminal queue. Active or quarantined allocation
rejection still comes from the existing owner ledger, not the Local-native check.

Runtime input is rooted before driver selection. Native failure diagnostics
remain typed until returned custody is rooted. Retry requires the DeviceLocal
synchronous placeholder and returns Quiescent, preserving prior scrub semantics.
The original box allocation is reused for restoration, with no new box
allocation needed by demotion or retry. Error-message formatting may still
allocate. Allocation removal, requested-byte refund and profile release remain
after successful recycle.

`take_restore_shell_v1` adds one reviewed unsafe block: safe Box APIs do not
split initialized value ownership from a reusable allocation. The unique Box
pointer is read once, then reconstructed as Box<MaybeUninit<T>> with identical
layout/alignment and allocator provenance, including aligned zero-sized types.
There is no fallible operation, callback or destructor between those moves.
The shell cannot drop the moved value and is refilled through Box::write only.
Independent static review and non-Copy/drop-order/unwind/alignment/ZST tests
support this contract; they are not a formal Rust memory-model proof.

## CPU Evidence

Eight new constructed tests use genuinely allocated and promoted mappings and
the original directional pair. The model loan/reclaim fixture uses actual
foundation transitions. Coverage includes 17/4097-byte and padded extents,
healthy retry followed by disposal, the 36-case validation/retake/secondary-poison
matrix, opening failures, active Reserved/Prepared use cancellation, generation
overflow/quarantine/zero-debit rejection, and real foreign-mapping rejection.
Failure snapshots compare the actual native mapping identity, Rc incarnation,
ledger allocation address/records/counters/frontier/quarantine, attachment and
memory/account/control-resource state. Two-parent mapping rejection restores
each fixture's own trace before original-owner cleanup.

Public lower admission and Drop probes carry genuine fixture allocations in
engine-less session shells. They exercise foreign-first/disabled/terminal
rejection and retained-root guards, not successful constructed public admission.

Nine new runtime tests cover original box reuse, exact device/neighbor/shadow
identity, authentic last-write invalidation by actual scripted scrub, retry and
once-only refund, independent missing/kind/occupied-slot rejection, lower/driver
selection/typed-diagnostic panics, public configured/unconfigured Context credit
settlement and core-disabled subprocess Drop. Terminal retries compare indexed
and retained owners, requested accounting and the exact profile event roster.
Unconfigured Context seals on its next backend call following panic; the backend
is already terminal. These remain injected CPU/scripted outcomes, not GPU faults.

The final focused run passes eight KFD and eleven runtime tests (nine new,
two existing demotion regressions). Strict all-feature/all-target Clippy,
formatting and runtime no-default-feature compilation pass. Unsafe-source policy
passes five tests with its explicit inventory-maintenance test ignored. The
reviewed baseline changes only runtime backend unsafe-block count from one to
two; unsafe impl/trait counts are unchanged. The first compile failed on
test-only Debug bounds and a pair-field spelling; its log is retained. Preliminary
passing runs predate strengthened assertions. Initial all-workspace formatting
overlapped the first failed build; final qualification uses frozen source.

| Target | Crate | Passed | Failed | Ignored | Duration |
| --- | --- | ---: | ---: | ---: | ---: |
| GNU | KFD | 1,352 | 0 | 0 | 1,160.13 s |
| GNU | Runtime | 798 | 0 | 6 | 29.72 s |
| musl | KFD | 1,352 | 0 | 0 | 1,682.11 s |
| musl | Runtime | 798 | 0 | 6 | 35.14 s |

The six ignored tests are opt-in hardware tests executed separately below in
eight isolated processes. GNU and musl lower runs overlap on the local host,
with four test threads each. Durations are provenance, not benchmark results.
This is not full-workspace or full R126 source-gate/compiled-negative/proof
qualification. No Miri run is claimed; that component is not installed.

Commands:

```sh
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --lib --all-features sdma_demotion -- --test-threads=4
prlimit --core=0:0 -- <gnu-kfd-test> --test-threads=4
prlimit --core=0:0 -- <gnu-runtime-test> --test-threads=4
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --target x86_64-unknown-linux-musl --lib --no-run
prlimit --core=0:0 -- <musl-kfd-test> --test-threads=4
prlimit --core=0:0 -- <musl-runtime-test> --test-threads=4
cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo check --locked --offline -p fe2o3-runtime --no-default-features
cargo fmt --all -- --check
cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
```

The final focused-build log identifies the exact GNU executables used for the
complete runs; `musl-build.log` identifies the musl pair. Two independent static
reviews cleared lower/runtime changes after strengthening the mapping, ledger,
slot, scrub and terminal-retry oracles. Reviews are not execution or proof.

## Native Evidence And Cleanup

All eight sequential isolated probes pass on MI300X GPU 1, unique ID
`ab83d2ffef0d3cdf`, using the stripped musl runtime binary with SHA-256
`13ca7f8a8039e214f91069335b614cbab382f4d28134b1ce6dde7cf5a483526a`.
SCP completed with status zero and the completed remote hash matched before
any probe ran. Each invocation used the explicit ID, `prlimit --core=0:0`,
`timeout --signal=TERM --kill-after=10s 120s` and
`--exact <test> --ignored --nocapture --test-threads=1`.

- Zero-cache and default-cache patterned device roundtrips validate 17/4097
  logical bytes on 4096/8192 physical bytes through allocation, upload, handled
  internal download, public release and retained shutdown. These success paths
  exercise the new demotion driver. The zero-cache case refunds 12,288 backing
  bytes/two device records before trim, with zero retained/reserved/quarantined
  device remainder. The default-cache case retains four buffers before trim;
  trim refunds the same backing. Both verify eight-event runtime histories.
- HostVisible allocation/shutdown and primary/two-stream vecadd regressions
  pass, with three/six readbacks and 22/41-event validated profiles.
- AUX budget failure with `FE2O3_TEST_NATIVE_INITIALIZED_PREFIX=0/1/2` retains
  38,281,216/42,475,520/46,669,824 backing bytes and 12/13/14 records respectively.
  These are process-exit-only retention observations, not successful cleanup.

The patterned readback SHA-256 values are
`cca448791d4bcee8fe07acb2b42c1ea727893454bf2d47cfbd43b644d9c6fd76`
(17 bytes) and
`7d9f8ee9ea61461d10b2b71c408cb7153390786855259a9ded5872aa87b514e3`
(4097 bytes). Vecadd output is
`79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.
Host control backing is 532,480 bytes/three records immediately before retained
shutdown and zero afterward. These probes add native success/regression evidence,
not native demotion failure injection, copy/compute overlap, machine-code
refinement or matched HIP/HSA timing evidence.

Only the stripped executable was uploaded to `/tmp/fe2o3-r126-demotion.rcFEpc`.
After all probes exited zero, an anchored query found no matching executable
(recorded exit 1). The exact file and empty directory were removed and absence
verified. GPU 1 reports zero use/VRAM percentage before and after. GPU 0 had
unrelated VRAM use of 63% before and 66% afterward; no reset, service stop or
unrelated deletion was performed. Raw receipts record these observations.
The local stripped copy was also removed after the binary recheck. No executable
is included in this archive.

Invocation flags, SCP completion and execution ordering above are operator-recorded
details. The raw outputs corroborate test results, hashes, accounting and cleanup,
but do not independently authenticate every command or its ordering. This is a
development receipt, not a closed full-qualification transcript.

## Remaining Scope

Synchronous-copy ownership across preparation/publication/wait/retirement,
typed healthy capacity rejection and borrowed owner-roster preflight remain.
Allocation during pending compute stays disabled. Generated DATA-ADOPT,
ISSUE/COMPLETE, readbacks/typed replies, Stop/drain/graphs, production Context
integration/resource proofs, other queue profiles, native faults, aggregate
memory, multi-device/distributed behavior, device-language/collective refinement
and matched HIP/HSA performance remain part of the unchanged full objective.
