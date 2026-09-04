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

The separately versioned KFD Runtime Semantic Profile V1 sidecar leaves every
frozen Runtime Profile V1 byte and decoder unchanged. It classifies every
retained dispatch publication exactly once: `None` explicitly means ordinary,
while typed atomic records retain operation, scope, success order,
compare-exchange failure order and weak mode, and typed collective records
retain operation, scope, order, participants, and geometry. Each record binds
the exact Runtime Profile V1 content identity, publication event identity and
sequence, dispatch, opaque dispatch shape, and duplicated launch geometry.
Canonical decoding rejects missing, duplicate, reordered, stale, malformed, or
unrehashed substituted joins. A structurally rehashed sidecar remains only a
new authority-free claim with a different content identity; only runtime-owned
V2 custody authenticates it as the direct runtime producer's observation.

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
The additive V2 custody bundle additionally retains the exact semantic
sidecar. It is available only when semantic profiling was explicitly enabled;
the established V1 timestamp producer and finish path do not allocate or
validate sidecar state.
