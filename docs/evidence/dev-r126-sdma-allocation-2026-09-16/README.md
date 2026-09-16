# R126 Fresh SDMA Allocation Custody: Development Receipt

This is a prerequisite for safe allocation during pending compute, not admission
of that workflow. R125 remains accepted at the CPU/test boundary. R126, A1/A2,
issue #182, formal correspondence and HIP/HSA parity remain incomplete.

## Source

Base: `14f7681ad6727f0d5b4c873fb3fca21b625f599a`.
`source.patch` contains the complete KFD source/test delta; its SHA-256 is
`3e86bda46851ce26eb3a680f7090b5496ab96bf55aa74d905043a87b788daad5`.
Documentation is separate. `SHA256SUMS` binds the archived patch, toolchain and
binary identities, commands/results and this receipt. No binaries are committed.

The queue owns an optional host/device allocation root throughout the real model
loan, native allocation/mapping and retake. The operation returns no owning value
through retake. Successful retake precedes output extraction and the outstanding
counter commit. Device allocation uses the existing borrowed-map custody rather
than the old consuming map helper. Host failed transitions remain rooted in the
lower memory engine, while host completion followed by failed retake remains in
the queue slot. These are distinct retention locations, not duplicated authority.

Empty, healthy pre-effect rejections clear the slot. Lower quarantine, uncertain
native effects, returned owners, retake failure and panic retain it. Reentry and
teardown reject; dropping a session with an unfinished slot aborts. No native
rollback, terminal refund or retry is claimed. A local poison adapter preserves
the original panic even if poisoning panics. Device layout validation and error
nesting retain their original model-loan boundary and `Sdma(Memory(...))` form.
Disabled-SDMA allocation attempts still close pool configuration irreversibly.
The driver adds inline state, not a success-path heap wrapper; no allocation-count
benchmark, total-memory bound or throughput improvement is inferred.

## CPU Validation

The frozen GNU focused run passes 12 tests, comprising eleven constructed-parent
tests and one subprocess guard test. Matrices exercise lengths 1/17/4097, rounded
device backing with original logical length, exact native operation prefixes,
host/device budget rejection and successful retry, zero/invalid layouts,
outstanding overflow, opening errors/panics and genuine generation exhaustion,
retake-before/after errors/panics and real revision regression, lower native and
currentness faults, partial/malformed map results, first-panic precedence and
terminal reentry. Returned buffer branding, generation, content-certificate
absence, original primary resource/signal identities, account/record partitions
and actual authenticated loan placement are checked. Successful cases release
their buffers and directional/primary resources and observe complete refunds.

Constructed fixtures use configured accounts and scripted native operations.
They do not qualify unconfigured native failures, all queue profiles or physical
GPU overlap. The Drop test deliberately uses an engine-less session with an empty
root; it proves the public guard, not native-owner construction. Original SDMA
doorbell identities and process-gate invariance are not separately fault-matrix
oracles in this packet. Fixture-only anonymous SDMA mappings are cleaned after
observations without claiming a native teardown on failure.

GNU and musl runtime regressions each pass 764 tests, with four opt-in hardware
tests ignored locally. Full KFD GNU and musl each pass 1,326 tests with none
failed or ignored, in 1,040.10 and 1,581.36 seconds respectively. These are
crate-level regression results, not the full-workspace qualification campaign.
Strict Clippy, no-default-feature compilation, formatting and unsafe-source policy
pass; the unsafe inventory maintenance test remains explicitly ignored.

Commands:

```sh
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib --no-run
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --target x86_64-unknown-linux-musl --lib --no-run
# Freeze the four executables in a private directory, then run each:
<frozen-binary> --test-threads=4
<frozen-kfd-gnu> sdma_allocation --test-threads=2
cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo check --locked --offline -p fe2o3-kfd -p fe2o3-runtime --no-default-features
cargo fmt --all -- --check
cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
```

The initial exploratory compile used three nonexistent test accessor names and
was corrected to existing APIs. The first five-test run passed three and failed
two: poison could replace the first panic, and an overly strict snapshot assertion
ignored normal model checkpointing during a loan cycle. The production poison
adapter fixes the former; the test now separately checks unchanged native state
and the expected model checkpoint. These exploratory outputs are not archived
qualification logs. An intermediate focused run passed ten tests; final coverage
adds layout and actual backing-credit rejection. No full-workspace, compiled
negative, calibration or authenticated Verus campaign is claimed here.

Independent bounded read-only reviews found no blocking source or scope defect.
The archived KFD patch was compared with the working source delta; the focused,
GNU KFD, GNU/musl runtime and six native results were checked against this receipt.
Those reviews preceded the final musl KFD result and archive manifest. They are
not a full R126 qualification review or formal implementation correspondence.

## Native Regression

Stripped musl SHA-256:
`47be94971b66ec2cc4644f3a7f49598ebc3ea8b7932410608757fa3413a5c05a`.
The uploaded hash matches. Six sequential isolated processes on MI300X GPU 1,
unique ID `ab83d2ffef0d3cdf`, pass using the unchanged opt-in runtime probes:

- Public allocation/pool return and packetless retained teardown.
- Exact-artifact vecadd on primary ordinal 0, three full-buffer readbacks.
- Preallocated two-stream vecadd on primary 0/AUX 1, six full-buffer readbacks.
- AUX host-budget rejection after exactly 0/1/2 materialized owners while the
  first dispatch remains logically pending; repeat flush is inert.

Both vecadd success probes retain output SHA-256
`79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`,
complete 22/41-event profile histories and final host-account refund from
532,480 bytes/3 records to zero. AUX budget failures retain resources until
process exit, with exact charges/prefixes described in the preceding
[materialization receipt](../dev-r126-runtime-materialization-2026-09-16/README.md).
These are native regressions, not new allocation-retake fault injection,
DeviceLocal native qualification, terminal refund or physical-overlap evidence.
Profile timings are observations, not matched HIP/HSA performance measurements.

Each command used a single exact test, `--ignored --nocapture --test-threads=1`,
`FE2O3_TEST_NATIVE_UNIQUE_ID=ab83d2ffef0d3cdf`, core dumps disabled and
`timeout --signal=TERM --kill-after=10s 120s`. Budget probes additionally set
`FE2O3_TEST_NATIVE_INITIALIZED_PREFIX` to 0, 1 or 2.

## Cleanup And Remaining Work

All six native processes exited successfully. Only the uploaded executable was
removed from `/tmp/fe2o3-r126-sdma-allocation.XquyXI`, then its directory was
removed with `rmdir`; absence was checked. The first process query had a remote
shell quoting error, preserved in `native-process-check.log`; the corrected
anchored query returned exit status 1 with no matches and is recorded separately.
GPU 1 remained at zero reported utilization
and VRAM percentage afterward. GPU 0's existing 44% VRAM allocation was unchanged.
No resets, service stops or other users' files were involved.

After all four local suites completed, the five frozen scratch executables were
removed; logs, source patch and binary hashes were retained. The evidence archive
contains no executable copies.

Next: root runtime HostVisible and upload-staging buffers around borrowed writes;
root consuming promotion through validation/retake and diagnostic conversion;
harden DeviceLocal synchronous-copy ownership. Then introduce borrowed live-owner
preflight for existing directional SDMA while keeping creation idle-only, and
qualify each allocation kind with pending compute. The runtime currently converts
all lower allocation errors to terminal string diagnostics, including healthy
backing-credit capacity rejection. Preserve that proven pre-effect distinction
through a typed classification with healthy settled-state checks; string matching
or a blanket resource-credit-error downgrade is not sufficient. The runtime call
may already have created primary/directional queues before the lower rejection,
so a lower pre-effect error alone does not establish whole-call `Rejected`
semantics. Initially scope recovery to existing genuine directional owners, or
separately qualify creation-budget preflight. Upload/download staging and partial
chunk effects need their own disposition analysis. All larger R126 qualification,
generated-kernel, graph, resource-proof, multi-device and matched-performance
requirements remain as tracked in the runtime roadmap.
