# R126 Lower Synchronous SDMA: Development Receipt

This packet extends `d61977836b45cec793f0cb3db6145ef30fe1a1a4`.
R125 remains the accepted CPU/test checkpoint. R126, A1/A2, issue #182,
formal implementation correspondence and HIP/HSA parity remain incomplete.
Both full KFD/runtime regression pairs below completed successfully.

The complete source/test delta is `source.patch`, SHA-256
`c1fdbecec5718431c84860c2b1333b0b64e7d710465d0a8e1d462754361437e5`.
`source-files.sha256` records the fifteen modified/new Rust files. Documentation
is separate. Source is frozen for the final builds, full regressions and native
probes. Executable identities are recorded in the GNU/musl binary manifests.

## Ownership And Ordering

The lower public synchronous-copy facade installs the original persistent
allocation, host buffer and typed lease state before opening currentness.
Preparation borrows the request through mapped-resource callbacks. Publication
installs both buffers in the original selected queue record, and installs the
retained ticket in the caller root, before completion clearing, ring writing,
write-pointer publication or doorbell access. Successful final currentness
precedes record extraction; completed buffers are rooted before model retake.

The original separate opening loan and single fused prepare/publish/wait loan
remain. Retake precedes native restoration, completion/settlement and owner
return. Failed pure transitions reinstall returned leases. Timeout returns the
published submission while the actual queue keeps both buffers. First panic
survives retake and poison panics. Ordinary terminal errors seal the session;
the unwind/failed-retake envelope additionally poisons process admission.
Publication, completion and currentness quarantine causes remain distinct.

The mapped-memory trait forwards native calls without a second queue algorithm.
The wait algorithm and packet constructor remain shared. No success-path heap
allocation or unbounded scan is introduced. Terminal payloads remain inline,
including an unsettled lease when quarantine itself rejects. This increases
some enum sizes; stack/performance effects are unmeasured. No new unsafe block
is introduced.

## CPU Evidence

Fourteen new constructed tests use original directional queues, genuinely
allocated host/device mappings and real foundation loan/reclaim transitions.
They cover both directions and nonzero offsets; exact mapping-derived packet
bytes, pointer progression and untouched sibling state; healthy preparation
retry/cancellation; preparation/publication/wait errors and panics; timeout and
wrong completion; original-panic precedence; actual operational-currentness and
mapped-backend failures; returned-metadata corruption; retained foreign leases;
and inert public reentry with a core-disabled subprocess Drop guard.

Device completion is simulated by writing the actual fixture completion mapping,
then running the production wait. These tests do not simulate GPU data movement
or establish hardware atomic ordering. Metadata/lease corruption is explicitly
injected after real lower execution. Public guard probes install genuine
allocations in engine-less session shells, not fully native public parents.
Recoverable publication rejection followed by failed restoration has static
review but no direct constructed regression in this packet.

The actual backing-account behavior is preserved: ordinary operational
currentness errors quarantine memory; direct operational-currentness panics
leave its phase unchanged while the parent is terminal. Configured coherent
completion-access panics quarantine memory, unlike ring/userptr-control access.
These cases retain their original panic and actual owners in both cases.

| Target | Crate | Passed | Failed | Ignored | Duration |
| --- | --- | ---: | ---: | ---: | ---: |
| GNU | KFD | 1,366 | 0 | 0 | 1,836.39 s |
| GNU | Runtime | 798 | 0 | 6 | 30.64 s |
| musl | KFD | 1,366 | 0 | 0 | 2,978.47 s |
| musl | Runtime | 798 | 0 | 6 | 34.25 s |

Strict all-feature/all-target Clippy, formatting and runtime no-default-feature
compilation pass. Unsafe inventory policy passes five tests with its explicit
maintenance case ignored. GNU/musl KFD runs overlap on the local host with four
test threads each. Durations are execution provenance, not benchmarks.

Two bounded read-only reviews found no blocking source defect, confirmed the
complete staged patch and all fifteen source identities, and checked the
completed build/runtime/native evidence. Their static ownership and efficiency
reviews are not execution or formal proof. The direct restoration-coverage gap
above remains explicit.

Preliminary logs retain a failed test-only compile, missing frontier retirement
in fixture cleanup, an opening-fixture poison-envelope mismatch, and an
overbroad quarantine oracle. They were corrected rather than accepted. The
initial Clippy run rejects inline enum sizes and large retained errors; explicit
allocation-free custody annotations address these findings. Initial Clippy
overlapped formatting, and preliminary test runs predate final test additions.
Only the final frozen-source runs support the current source. The preliminary
238-test SDMA run passed before the last two tests and stronger packet oracle.

Commands:

```sh
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib --no-run
prlimit --core=0:0 -- <gnu-kfd-test> --test-threads=4
prlimit --core=0:0 -- <gnu-runtime-test> --test-threads=4
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib --target x86_64-unknown-linux-musl --no-run
prlimit --core=0:0 -- <musl-kfd-test> --test-threads=4
prlimit --core=0:0 -- <musl-runtime-test> --test-threads=4
cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo fmt --all -- --check
cargo check --locked --offline -p fe2o3-runtime --no-default-features
cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
```

## Native Evidence And Cleanup

Eight sequential isolated MI300X probes passed on GPU 1, unique ID
`ab83d2ffef0d3cdf`. The stripped musl runtime binary has SHA-256
`e775f4765fb8642e5e6d58ba48148f5312e0302719667691554823820e92c40f`.
SCP completed with status zero; the remote hash matched before execution and
again after the probes. Each invocation supplied the unique ID, disabled core
dumps and used `timeout --signal=TERM --kill-after=10s 120s`, with an exact
ignored test and one test thread.

Zero-cache and default-cache patterned 17/4097-byte device roundtrips exercise
the new lower synchronous driver in both directions, including restoration and
frontier retirement by the existing runtime. Host allocation/shutdown, primary
vecadd and two-stream vecadd regressions also passed. AUX budget rejection with
initialized prefixes 0/1/2 remains separate process-exit retention evidence,
not successful native fault cleanup. These are success/regression checks, not
native copy-fault injection, copy/compute overlap or matched HIP/HSA timings.

Only the stripped executable was uploaded to
`/tmp/fe2o3-r126-synchronous.p6aLhP`. After the probes, an anchored process query
found no matching executable (exit 1). The exact executable and empty directory
were removed; `stat` confirms absence. GPU 1 reports zero use/VRAM percentage
before and after. No GPU reset, service stop or unrelated deletion was used.
The local stripped copy was also removed; no executable is archived here.

Invocation flags and ordering above are operator-recorded. Raw logs corroborate
test results, hashes, accounting and cleanup, but this is not a closed
full-qualification transcript, authenticated proof or full-workspace run.

## Remaining Scope

The runtime outer synchronous path still needs rooted normalization, typed
diagnostics, original-box restoration, completion retirement and transient
readback. Typed healthy capacity rejection, pending-compute allocation and other
SDMA profiles remain open. Full R126 source gates, compiled negatives and
qualification are still required. Generated DATA-ADOPT, ISSUE/COMPLETE, typed
replies, Stop/drain/graphs, production Context/resource proofs, multi-device
behavior, device-language/collective refinement and reproducible matched HIP/HSA
performance remain part of the unchanged objective.
