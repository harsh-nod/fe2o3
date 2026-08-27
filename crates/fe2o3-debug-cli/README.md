# fe2o3-debug

`fe2o3-debug sim --kir-v7 KERNEL.kir --request REQUEST.json` opens a bounded
JSONL debugger over an exact deterministic CPU simulation transcript. Requests
arrive on standard input and one `fe2o3-debug-response-v1` line is written for
each request. `--protocol jsonl` is accepted explicitly and is the only V1
transport.

`fe2o3-debug sim --bundle KERNEL.fe2sim --request REQUEST.json` securely
decodes and revalidates the authority-free compiler simulation bundle, then
uses only its exact embedded KIR V7 and target. `--bundle` and `--kir-v7` are
mutually exclusive. An embedded debug map is admitted from those same captured
bundle bytes and labeled `compiler_bundle_bound`; callers cannot override it
with `--source-map`. This label proves exact bundle content association only,
not compiler execution, source authorship, hardware observation, or authority.
When a bundle has no map, source features remain typed `unavailable`.
Compiler-exported bundles normally contain a map derived from the same rustc
and semantic-KIR transaction. V1 exposes resolved call-site spans only;
synthetic KIR operations, source variables, and macro expansion stacks remain
typed unavailable rather than being inferred.

The simulator exposes work-item, logical wave, workgroup, KIR operation, SSA,
allocation-relative ordinary and integer-atomic memory, fence order points,
barrier, and committed-memory observations. Atomic watchpoints match one
indivisible atomic record; read/write watchpoints include atomic records with
the corresponding effect. Reverse navigation is deterministic transcript
replay; forward stepping includes
frame-aware over/out. It is not GPU reverse execution;
logical waves are visualization partitions, not hardware wave observations.
Captured F16/BF16/F32/F64 scalar values retain their exact software-IEEE bits;
the debugger does not render or recompute them with host floating-point
arithmetic.

Barrier residency is replayed from semantic records. A lane is
`barrier_blocked` from its arrival through the record before the matching
workgroup release. A wave or workgroup is `barrier_blocked` only when every
active lane in that aggregate is waiting; a partial wait is `runnable`.
Dispatch aggregation follows the currently scheduled workgroup. The release
record clears residency before scope state is reported, so its representative
lane is `running` and other released lanes are `runnable` until their next
record.

`--source-map MAP --source-bundle-subject SUBJECT` admits a strict, bounded
`fe2o3-debug-source-map-v1` sidecar. Both options are required together. The
map binds the canonical KIR digest and length plus a non-circular
compiler-bundle subject identity. The wire map binds these compile-time facts
only. Admission derives the complete runtime simulation configuration from the
admitted request and wave width, then the backend rechecks that internal
binding before use. Bundle sessions additionally bind the verified bundle
subject, which commits the exact target. A reusable compiler map must not claim
a future request or wave configuration.
Files are content identities and display paths only; the debugger never opens
a source path from the map. KIR/source resolution, source breakpoints, and
source stepping return distinct absent, eliminated, and many-to-one states.

This command-line pair is a low-level/test consistency boundary. Because the
caller supplies both documents, it is labeled `caller_bound`, not compiler
provenance. Production compiler integration must call
`run_admitted_jsonl_with_compiler_source_map_v1` with exact map bytes, the
verified bundle subject, and the bundle-committed map identity obtained from the
same compiler extraction/bundle decode transaction. Only that path emits
`compiler_bundle_bound`. This proves exact bundle content association, not
protected compiler execution. The bundle subject excludes the map payload
to avoid circular identity; the enclosing bundle identity commits the map's
domain-separated identity and exact length. `debug_source_map_identity_v1`
implements the bundle's exact map-identity algorithm.

`inspect_stack` pages captured simulator call frames, including function/block
ordinals, the next operation, and typed captured/unavailable value state.
Frames are not reconstructed from names or UI fixtures. Source variables,
hardware registers, hardware wave state, and KFD dispatch control remain typed
`unavailable`; no value is fabricated. KIR, request, and sidecar files use the
hardened regular-file capture boundary shared with `fe2o3-kir-sim`.

## KFD hardware protocol V2

`fe2o3-debug hardware -- PROGRAM [ARG...]` launches that exact argument vector
as a ptraced child and exposes a separate bounded
`fe2o3-hardware-debug-request-v2` JSONL protocol on standard input. The
coordinator retains the target pidfd, owns the KFD debug-trap session on its
spawning task, and kills and boundedly reaps the launch-owned target on EOF,
protocol/output failure, or `terminate`. It invokes no shell, HIP, or HSA.

V2 provides redacted device and queue snapshots, bounded exception events,
and queue suspend/resume after the target enables its KFD debug runtime.
Runtime transition events are recorded and acknowledged internally so the
target-side KFD handshake can proceed. Device and queue identifiers are
session-local generation/ordinal pairs; the wire never contains a PID,
descriptor, native KFD identifier, target virtual address, or target argv.
Control revision and asynchronous observation sequence are independent.

The hardware protocol does not provide wave or lane state, register or CWSR
decode, stack/source/KIR sites, stepping, replay, breakpoints, values, target
memory, semantic trace, address watch, or dispatch submission. These
capabilities are explicitly reported unavailable and are never inferred from
the CPU simulator. The hardware path observes KFD state only; it does not
claim timing, performance prediction, race freedom, or GPU scheduling detail.
