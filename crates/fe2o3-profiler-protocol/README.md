# fe2o3-profiler-protocol

This authority-free crate owns canonical, bounded observations emitted by the
direct-KFD runtime and a stable read-only JSONL query contract. It records
logical runtime resource lifecycle, host staging reads and writes, exact
artifact and launch identities, native AQL publication, completion, and
host-monotonic phase durations.

Host staging records always bind allocation-relative ranges. The capture
explicitly declares whether payload content identities were requested or the
low-overhead range-only policy was used.

The capture does not claim GPU clock timestamps, DMA/copy-engine events,
hardware counters, PC samples, decoded ATT, rocprofv3 dispatch correlation, or
authenticated source/IR/ISA attribution. Those facts are returned as typed
unavailable capabilities until independently collected evidence can be joined.
No record in this crate grants compiler, proof, load, dispatch, or native-handle
authority.
