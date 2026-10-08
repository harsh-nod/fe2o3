# Native Packet Diagnostics

`engineering-native-packet-diagnostics` is a default-off engineering feature.
It includes `engineering-native-wait-diagnostics`. It is not a supported
serving mode, completion authority, or a performance optimization.

For each whole-program execution it emits a
`Fe2o3NativePacketObservationV1` JSON line to stderr, after successful native
completion and signal retirement. `packets` contains `[kernel, start, end]`
triples in original packet order. `symbols` binds used kernel handles to their
loaded metadata symbols. `first_write`, `next_write`, device identity and queue
epoch identify the exact publication.

The diagnostic uses the existing ROCr 6.4.3 queue profiling bit and signal
timestamp ABI. Its distinct program arena supports 1 through 1024 signals;
the ordered64 page and limit are unchanged. It clears only timestamp fields,
before publication on a current idle queue. It does not add dispatches, split
the program, change fences, or alter the final-signal polling policy. Capture
and property restoration happen only after normal completion, all-signal
retirement and idle/currentness checks. Any failure is terminal and poisons the
queue; uncertain execution is never followed by restoration or reuse.

Timestamps are raw GPU clock ticks. They describe firmware packet-processing
intervals, not necessarily shader-only execution. No frequency conversion,
non-overlap or cross-device clock synchronization is inferred. Consumers must
retain overlap, account for interval unions when describing uncovered gaps,
and separately validate model output, launch identity and clean shutdown.
Enabling firmware profiling and collecting/serializing diagnostics perturbs
execution; this capture cannot establish uninstrumented TTFT or TPOT.

The CPU wait record is sampled before packet capture/serialization. Its
execution-return field therefore excludes the later packet-diagnostic work.
The enclosing controller execution includes it.

ABI references:

- [ROCr signal layout](https://github.com/ROCm/ROCR-Runtime/blob/rocm-6.4.3/runtime/hsa-runtime/inc/amd_hsa_signal.h)
- [ROCr queue profiling property](https://github.com/ROCm/ROCR-Runtime/blob/rocm-6.4.3/runtime/hsa-runtime/inc/amd_hsa_queue.h)
