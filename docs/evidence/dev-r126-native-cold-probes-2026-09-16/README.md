# R126 Native Cold-Capacity Probes: Preparation Receipt

This packet extends `5774053889f6561b7a776250af6944324527bb9e`.
It adds two ignored native probes and one environment-free CPU guard test;
it changes no production allocation behavior. The native probes are compiled,
not executed. R125 remains the accepted CPU/test checkpoint; R126, A1/A2,
issue #182, formal correspondence and HIP/HSA parity remain incomplete.

`source-files.sha256` binds the two changed Rust test files. The complete
`source.patch` SHA-256 is
`3ac5085fb2b8e20ea5164d2c668cf3bea65c7f1927d54ede0325bded2a59f9b5`.

## Native Probe Contract

Each probe requires explicit `FE2O3_TEST_NATIVE_UNIQUE_ID` and exact
`FE2O3_TEST_NATIVE_ISOLATED=1` before opening a native device. The latter is an
operator acknowledgement, not proof of exclusivity. CPU tests pass strings
directly into the guard; they never mutate environment variables or access GPUs.

Both probes configure a 16 MiB/32-record Host backing account, a 4096-byte/one-record
Device backing account and zero-capacity Host/Device caches before any resources.
Context admission permits the full failing request and one allocation record.

- DeviceLocal requests 4097 logical bytes, requiring 8192 padded backing bytes.
- HostVisible requests 16 MiB plus 4096 bytes, exceeding the entire Host account.
- The first public allocation must return cold `BackendQuiescent(Capacity)`,
  leave actual queues/SDMA enabled, and refund the entire Context attempt.
- The same request must then return warm `BackendRejected(Capacity)` with the
  same diagnostic, primary queue/lane identity, accounts and pool observation.
  Attempt IDs are consumed; whole queue/model immutability is not asserted.
- A 4096-byte retry must succeed with the exact native owner, Context charge,
  backing-account delta and sole checked-out buffer under unchanged limits.
- Public zero/pattern reads and writes are checked against native-adapter reads.
  A controlled adapter-only write then changes native bytes while the retained
  host shadow remains unchanged; public read must return those different bytes.
  A public write reconciles shadow/content metadata before release. This is
  mixed facade/adapter qualification, not an all-public application sequence.
- Public release must restore exact Context, Host, Device and pool baselines.
  Existing retained-primary shutdown observations now also capture the Device
  account before and after root cleanup, rather than treating unavailable usage
  after Drop as proof of a refund. Repeated shutdown must be inert.

These are deterministic session-account rejections, not global GPU exhaustion
or injected allocation-ioctl failures. They cover the zero-cache profile only.
The Host control baseline is observed dynamically, not presented as a new
aggregate bootstrap-memory bound. No kernel launch or performance measurement
is part of these probes.

## Hardware Availability

`isolation/` preserves the read-only UTC/ROCm observation. All eight MI300X
devices had attached processes and nonzero VRAM. GPU 0 reported zero utilization
but held 91,545,174,016 bytes; GPUs 1-7 were busy. The separate process-detail
log was intentionally excluded because it contains other users' workload paths
and arguments; the retained ROCm log independently includes GPU/PID mapping.
No remote files, uploads, resets, deletions or test processes were created.

## CPU Validation

The final runner completed all 17 recorded commands successfully. GNU and musl
runtime library suites each pass 841 tests with zero failures and ten ignored
hardware tests (24.26 and 26.60 seconds respectively). Both new cold-capacity
probes appear in each ignored roster; the environment-free guard runs normally.
Strict all-feature/all-target runtime Clippy, workspace formatting, runtime
no-default-feature checking and source checks before/after all pass. The unsafe
source policy passes five tests with one explicit maintenance test ignored.

`final/` retains commands, UTC start/end times, exit codes, raw output, Cargo
JSON executable selection and executable hashes. These are runtime-library CPU
results, not a full-workspace campaign or native execution. The preliminary
digest-format compilation failure and its successful retry remain in
`preliminary/`; neither substitutes for the final-source validation.

## Reproduction

`bash verify-cold-probes.sh CHECKOUT OUTPUT_DIRECTORY` runs only CPU validation.
The output directory must exist. Requirements are locked offline dependencies,
GNU/musl Rust targets, Python for the existing Worker tests, `jq`, and GNU
`prlimit`. The runner binds source endpoints and Cargo JSON executable identities,
records executable hashes, and confirms both probes appear in the ignored roster.
It is not a full KFD or full-workspace qualification campaign.

After a fresh PID/VRAM isolation check and explicit device selection, run each
native probe in a separate isolated process with core dumps disabled. The
environment variables above must already be set. For example, with
`TEST_BINARY` set to the recorded musl runtime test executable:

```sh
prlimit --core=0:0 -- "$TEST_BINARY" \
  kfd_backend::retained_release_tests::cold_allocation::native_runtime_cold_device_capacity_refunds_context_and_retries \
  --exact --ignored --nocapture --test-threads=1
prlimit --core=0:0 -- "$TEST_BINARY" \
  kfd_backend::retained_release_tests::cold_allocation::native_runtime_cold_host_capacity_refunds_context_and_retries \
  --exact --ignored --nocapture --test-threads=1
```

Do not run these commands while other jobs own the selected GPU. Stop and retain
the original failure evidence if either probe fails; do not reset shared hardware.
No binary is included in this archive. `SHA256SUMS` covers all other members.
