# MI300X direct-KFD/rocprofv3 qualification, 2026-09-03

This is a bounded qualification of one exact run. It does not establish a
universal ROCprofiler SDK limitation, prove that no GPU activity occurred, or
grant collection or dispatch authority.

## Retained artifacts

- `mi300x-direct-kfd-rocprof-qualification-v1.json` is the canonical
  `fe2o3-direct-kfd-rocprof-qualification-v1` record. Its raw identity is
  `3d38dac937349180f1772d17ac54675a83f9b56fdfc75368ee0f7270a8432151`
  over 2,717 bytes.
- `mi300x-direct-kfd-runtime-profile-v1.json` is the exact canonical runtime
  capture copied from the retained source descriptor into the profile output
  transaction. Its raw identity is
  `1563981f4b265f9961f2ae36afc5388aaae5fe0ac7b9be2c67cf7c2186fed7b0`
  over 12,229 bytes.

The qualification record binds the exact plan, collector executable and
reviewed closure, collector configuration, argv, cleared environment, target
ELF and argv, bounded stdout and stderr, complete collector-output inventory,
collector exit, and runtime capture. It contains no executable path, native
address, descriptor, queue token, or runtime handle.

## Exact environment

- Host: Linux `6.8.0-124-generic`, eight KFD-observed `gfx942` Wave64 agents.
- Collector: `/opt/rocm-7.2.4/bin/rocprofv3`, ROCprofiler SDK `1.1.0`, git
  revision `97f5574fe2fdc7bef44fb01545347912ee9f1779`, ROCm `7.2.4`.
- Collector script: raw SHA-256
  `195ff5e6faf48a3abbc6f4db9f69dd598fe71fa9ff695ba2556d65af636fdc48`,
  62,506 bytes.
- SDK tool library: raw SHA-256
  `478df9af09b74707652d9d5574ef37151ff1234d09c68843972c1409a505cdd0`,
  5,435,848 bytes.
- SDK core library: raw SHA-256
  `40cf6fefffa5e9e8da249dbc1ce6feab0bb2613438caf37b0224d7fce09241e1`,
  8,314,944 bytes.
- Target: `gfx942-runtime-vecadd-benchmark`, raw SHA-256
  `3ea65bf68139ebf0d563e5d74f1e9de35bfe2fb93a1a019afa85c935d2cebb8b`,
  2,691,648 bytes. Its ELF dynamic dependencies were only `libgcc_s.so.1`,
  `libc.so.6`, and `ld-linux-x86-64.so.2`; the application path did not link
  HIP or HSA runtime libraries.
- Limits: 90-second timeout, 1 MiB stdout, 1 MiB stderr, and 64 MiB total
  output storage.

## Observed result

The target completed exact readback validation and emitted a complete direct-
KFD runtime capture with 31 events, zero dropped events, and three dispatches
published, completed, and released. The collector exited successfully. Its
bounded stdout identity covers 2,144 bytes and its bounded stderr identity
covers 985 bytes; neither overflowed. The complete collector-output inventory
contained zero artifacts.

The typed outcome is therefore
`runtime_dispatch_observed_collector_completed_no_artifacts`. Dispatch,
code-object, and clock joins remain unavailable because the two observations
share no admitted dispatch identity, runtime-to-code-object relation, or clock
relation. ATT and PC-sampling capabilities are `not_requested_or_probed` in
this record.

This outcome says only what the exact collector and target did in this run.
The installed SDK header
`dispatch_counting_service.h` (raw SHA-256
`06a432ca4f0b6c959880ca7c6defdc6f54139be515e3b6aed19eb4362ed782f9`)
describes counter selection at HSA-queue enqueue, and the installed HSA API
argument catalog exposes HSA queue interception. That makes an HSA-runtime
registration boundary a plausible explanation for the direct-KFD observation
gap, but it is an inference, not a universal SDK incapability claim.

### Empty-inventory investigation

A follow-up bounded probe used the same target and installed collector without
the fe2o3 sealed adapter. This rules out an adapter-only failure: the raw
installed entrypoint also exited successfully and produced no files with
`--kernel-trace --agent-index absolute --output-format json`. With
`--log-level trace`, the collector reported that it found and configured the
rocprofv3 client, registered and started five contexts, started the primary
context before invoking the target, flushed its buffer after the successful
target exit, and had zero services generating output. The bounded trace was
558,887 bytes with raw SHA-256
`222f2a1f14e10385d34df59927c0bf4619abda1c764e92aab468c8d50741b00d`.
It is not retained because it contains process-local addresses and identifiers.

Three negative controls distinguish an empty observation from an output-path
or threshold mistake:

- adding `--output-config true` wrote a 5,420-byte `capture_config.json` to the
  requested directory; its metadata recorded `kernel_trace: true`, JSON output,
  the requested output path and stem, and a zero-byte minimum-output threshold;
- explicitly adding `--minimum-output-data 0` still produced no trace artifact;
- replacing kernel trace with `--sys-trace` still produced no artifact.

The output-config file is configuration metadata, not dispatch evidence, and
must not be admitted as if it were a kernel trace. The installed 1.1.0 CLI also
rejects `--kfd-trace` as an unrecognized argument. Its installed SDK headers
describe dispatch-counting selection before enqueue into an HSA queue and
catalog `hsa_queue_create` plus `hsa_amd_queue_intercept_create`; they do not
expose a documented rocprofv3 path for registering a queue created directly by
KFD ioctls. The evidence therefore identifies no fe2o3 argument, output,
custody, or sealed-launch bug to fix. The narrow explanation is that this exact
rocprofv3 kernel-dispatch service had no registered/interposed runtime queue
from which to produce a dispatch record while the fe2o3 runtime independently
observed its direct-KFD dispatches. That explanation remains an inference about
this installed release. It is not proof that no custom SDK tool, future SDK, or
driver facility can observe a direct-KFD queue.

## Separate probes

A separate bounded ATT launch exited before the target with
`rocprof-trace-decoder library path not found`; no matching decoder library was
present under the installed `/opt/rocm-7.2.4` tree. This is an exact local
installation fact, not an ATT capability claim. The installed experimental
decoder header also requires code-object load facts from the SDK code-object
tracing service, which the direct-KFD runtime capture cannot supply as a common
identity.

`rocprofv3-avail info --pc-sampling` separately reported gfx942 host-trap/time
and stochastic/cycle configurations, with a minimum stochastic interval of
256 cycles. PC sampling was not run because its beta device-wide freeze risk
requires a separately authorized protected-runner experiment.
