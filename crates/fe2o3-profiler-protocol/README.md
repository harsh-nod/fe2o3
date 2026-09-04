# fe2o3-profiler-protocol

This authority-free crate owns canonical, bounded observations emitted by the
direct-KFD runtime and a stable read-only JSONL query contract. It records
logical runtime resource lifecycle, host staging reads and writes, exact
artifact and launch identities, native AQL publication, completion, and
host-monotonic phase durations.

Host staging records always bind allocation-relative ranges. The capture
explicitly declares whether payload content identities were requested or the
low-overhead range-only policy was used.

The frozen Runtime Profile V1 capture does not claim GPU clock timestamps,
DMA/copy-engine events,
hardware counters, PC samples, decoded ATT, rocprofv3 dispatch correlation, or
authenticated source/IR/ISA attribution. Those facts are returned as typed
unavailable capabilities until independently collected evidence can be joined.
No record in this crate grants compiler, proof, load, dispatch, or native-handle
authority.

The separate additive Dispatch Timestamp Capture V1 schema can structurally
admit an external producer claim against the exact Runtime Profile V1,
producer-declared evidence, and collection-configuration bytes. Each bounded record binds
dispatch, queue, device, kernel, and artifact identity; a CPU publication tick;
device start/end ticks; and before/after CPU/device correlation brackets. Raw
ticks remain opaque and the schema explicitly leaves device publication,
frequency/nanosecond conversion, global synchronization, and authenticated
producer provenance unavailable. Every tick and bracket is explicitly labeled
as a producer-declared observation, never as an authenticated `Observed` fact.
No production collector adapter currently mints authenticated records.

Native Runtime Dispatch Timestamps V1 is a separate host-only schema produced
inside the direct-KFD runtime profiler. Its clock is monotonic nanoseconds since
that recorder started. Each recorder obtains a fresh Linux `getrandom`
occurrence and binds it into the clock-domain identity, making accidental
aliasing across reused caller scopes and process-local epochs cryptographically
negligible. Every publication/completion point binds the exact retained runtime
event identity and sequence plus dispatch, queue, device, kernel, artifact,
capture-scope, and runtime-profile identity. Decoding its
canonical bytes validates structure and currentness only; native-runtime
provenance remains available solely through the non-constructible runtime
custody bundle. This schema has no GPU begin/end or device-clock fields.
