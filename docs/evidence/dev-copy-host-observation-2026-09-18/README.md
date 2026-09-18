# Copy Host Observation And Native Accounting

Development above `b87f30d1b87b2dca29e9f03e8b00f99a65b04391`. This work separates
runtime-owned backing accounting from external GPU telemetry. It does not
establish HIP/HSA performance parity, protected execution, formal refinement,
continuous host isolation, or a runtime memory leak. A1/A2 and #182 remain open.

## Observation Gap

The previous [failed comparison](../dev-kfd-native-wait-5d70cb0a-2026-09-18/README.md)
waited twenty seconds after its successful A/1 process before the postflight.
The VRAM check then returned before collecting PID mappings. Several other GPUs
also had elevated VRAM in that snapshot. Neither a runtime leak nor delayed
driver accounting follows from that record; the cause remains unresolved.
The later passing guard does not rehabilitate that attempt.

`benchmarks/runtime_gfx942/copy-host-observe.py` captures status and PID commands
unconditionally before evaluating admission. Three timestamped direct PCI-BDF
sysfs snapshots bracket the two commands. Both sysfs and SMI must match the
selected UID/BDF, GPU utilization must be zero, VRAM must remain strictly below
512 MiB, and the selected SMI GPU index must have no reported PID attachment.
SMI indices are not assumed to equal Linux DRM card numbers. Every failure is
retained; a later passing sample cannot override it. Visibility-filter environment
variables are removed for both CLI observations without changing the parent.

This host's `rocm-smi --showpidgpus --json` can return exit zero without PID
data. The observer instead requires a complete, strictly parsed text transcript;
unknown, empty, duplicate or malformed reports refuse admission. Raw stdout,
stderr, status, errors and timestamps remain available even on rejection.
The documented [AMDGPU sysfs counters](https://kernel.org/doc/html/v6.18/gpu/amdgpu/index.html)
are endpoint observations, not evidence of who owns memory. Per-process KFD
counter-file existence is not used as attachment evidence.

The GNU timeout wrapper requests a 20-second limit and a five-second kill grace;
the Python capture also has a 30-second timeout. CPU tests mock timeout behavior,
not real descendant cleanup. Output retention is capped after collection, not
an in-flight memory bound. The sampler executes only read-only SMI commands and
sysfs reads. It creates no GPU workload, files or reservation on the remote host.

Example invocation, requiring fresh observations for each campaign endpoint:

```sh
python3 -B benchmarks/runtime_gfx942/copy-host-observe.py \
  --gpu-index 4 --pci-bdf 0000:85:00.0 --unique-id 0x54f88318ca05093d
```

## Exact-Size Fixture

The opt-in native test
`kfd_backend::retained_release_tests::copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing`
uses the existing safe copy-only qualification constructor and default cache
policy. It requires `FE2O3_TEST_NATIVE_ACCOUNTING=1` and an explicit
`FE2O3_TEST_NATIVE_UNIQUE_ID`; this acknowledgement does not establish isolation.
Host/device backing budgets are 1 GiB/64 records and 256 MiB/one record.

The test allocates HostVisible/DeviceLocal/HostVisible buffers of 256 MiB each,
performs one full H2D/D2H pair with the benchmark's wait/flush continuation, and
checks every returned byte against a nonzero pattern after poisoning the
destination. Device initialization leaves one additional 4 MiB staging buffer
cached. This first candidate expected four final free buffers, 809,500,672 bytes,
and zero checked-out buffers. Its byte oracle incorrectly conflated original
buffer capacity with page-rounded backing; the native failure below disproves
that expectation. The preserved candidate is not an accepted native test.

The test does not pre-trim the cache. Actual shutdown must restore both ledgers
to their queue-only baselines after pool trim, then retire the primary root with
zero remaining host/device backing, records or uncertain dispositions. Repeated
shutdown must be inert. External VRAM baseline and performance are not asserted.
The constructor's four window-publication history entries are diagnostic
metadata, not a valid drain-capture or application completion receipt.

No production runtime algorithm, release policy, sandbox or authority is changed.

## Retained Runs

Nineteen CPU observer tests pass, including failed-status PID capture, exact
thresholds, identity changes, malformed transcripts, timeout partial output,
visibility filtering and sticky refusal. The initial six-sample read-only run
on GPU 4 at 08:14:11.641891666 through 08:14:29.117626356 UTC passed every endpoint:
all direct VRAM samples were 298,647,552 bytes and no selected PID was reported.
This is an observed quiet interval, not a reservation or a performance result.
Before/after observer-source hashes match. No remote scratch was created.

The first native-fixture build and `source-before` snapshot precede the added
disposition/explicit-opt-in assertions. They are preliminary records, not the
final qualified source. `source-qualified-before` starts the final source
identity interval. Unretained exploratory Python tests and formatting preceded
the retained campaign; their successful outcomes do not replace its receipts.

GNU/musl each pass 1,101 runtime tests with eighteen ignored, including this
hardware-only fixture. The existing R26 host guard's 75 CPU tests pass, as do
strict Clippy, no-default compilation, runtime formatting and unsafe-source
policy (five passed, one maintenance test ignored). Complete source identity
and the two actual test binaries remain unchanged during this CPU qualification.

## Native Oracle Failure

A separate single guarded native attempt ran the exact musl binary
`8e729393c536a7fdcb7ca42d81a667b55dc7b7163b37c5f27bdb6484698abd89`.
It aborted at the first staging-size assertion: the observed free-buffer capacity
was 4,194,272 bytes, not the asserted page-rounded 4,194,304 bytes. The explicit
H2D/D2H pair, full returned-byte check and shutdown assertions were not reached.
The immediate endpoint also refused its first VRAM sample; the later passing
endpoint does not rehabilitate either failure. This is a fixture-oracle defect,
not proof of a production memory leak or successful native accounting.

`candidate/copy_accounting.rs` preserves the exact rejected fixture. The source
snapshot and verification receipt in this archive describe that candidate;
the verifier requires its recorded source and executables, not later edits.
The corrected oracle must distinguish retained capacity from backing charge
and receive a new source/binary qualification and separate native attempt.
