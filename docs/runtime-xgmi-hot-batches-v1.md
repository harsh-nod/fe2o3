# Matched XGMI Hot Batches

This extends the persistent-hot benchmark producers, not production runtime
authority or the A1/A2 acceptance boundary. Native depth-16/32 qualification
and matched performance measurements remain required.

## Lifecycle

The KFD non-diagnostic `--aggregate-peer-batch-hot-only` mode and the HIP/HSA
`--persistent-hot` modes admit depths 1 through 32. Both KFD hot diagnostic
modes remain depth-one because their bounded diagnostic rosters are separate
contracts. Ordinary and ordered-segment paths are unchanged.

Each direction prepares every slot with a distinct payload pattern, guarded
source and independently guarded poisoned destination. One untimed prime batch
per direction precedes the warmups and samples. Timed HIP/HSA batches enqueue
every slot before waiting for any completion. HIP uses one nonblocking stream
per slot; HSA uses one signal per slot and resets every observed completion.
The original KFD path already submits the complete exact roster on its ordered
SDMA queue. No host initialization or readback interrupts the hot sequence.

After all timed batches, both native comparators read back and validate the
complete source and destination of every slot, including both guard regions.
A payload mismatch does not skip later buffers, slots or the other direction.
Native API failure retains the existing fatal-process error behavior; this
change does not add recovery or fault qualification.

The shared traversal helpers allocate nothing and are linear in depth. HIP
continues to reuse one synchronous host staging buffer. HSA retains its existing
per-slot upload/download allocations. Timed batches add no host payload copies.

## Reporting

`xgmi_peer_hot_results.py` accepts externally supplied trial controls and only
the matched depths 1, 16 and 32. It requires one exact, non-diagnostic backend
record; canonical ordered endpoint identities and gfx942 targets; the stated
lifecycle; positive ordered percentiles; and all applicable depth fields.
Guarded sizes, batch bytes and prime-round arithmetic must fit u64.

Each latency is for the whole one-direction batch. Bandwidth is
`copy_bytes * depth / batch_nanoseconds`, not one slot's bytes or both directions
combined. Dividing batch time by depth is an amortized cost, not an individual
copy latency percentile. Final canaries establish the final contents, not a
separate functional witness for each repeated intermediate copy.

The different host boundaries remain explicit: KFD facade enqueue through
aggregate close versus HIP/HSA native enqueue through observed completion.
Equal submitted depth does not establish equal physical engine concurrency.
HIP/HSA continue to report `runtime-selected-unknown`; KFD reports
`ordered-single-sdma`. Historical depth-one parsers and evidence are untouched.

## Qualification

The [CPU packet](evidence/dev-xgmi-hot-batch-cpu-2026-09-24/README.md) records
GNU/musl example tests, real native-comparator compilation, shared helper tests,
actual producer callbacks with instrumented CPU APIs, UBSan and expected-negative
mutations, and strict result parsing. Mock callbacks do not link GPU runtime
libraries and do not establish native scheduling, hardware support or speed.

The next native campaign must bind signed source and cold-built artifacts to
freshly admitted endpoints, use the declared depth/backend ordering, retain
each process/direction summary separately, check every buffer and explicit
teardown, and remove only its owned resources. No new Verus proof, whole-runtime
refinement, aggregate memory bound, A7 acceptance or HIP/HSA parity follows from
these benchmark changes.
