# fe2o3-debug

`fe2o3-debug sim --kir-v7 KERNEL.kir --request REQUEST.json` opens a bounded
JSONL debugger over an exact deterministic CPU simulation transcript. Requests
arrive on standard input and one `fe2o3-debug-response-v1` line is written for
each request. `--protocol jsonl` is accepted explicitly and is the only V1
transport.

The simulator exposes work-item, logical wave, workgroup, KIR operation, SSA,
allocation-relative memory, barrier, and committed-memory observations. Reverse
navigation is deterministic transcript replay; forward stepping includes
frame-aware over/out. It is not GPU reverse execution;
logical waves are visualization partitions, not hardware wave observations.

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
compiler-bundle subject identity. Admission binds those exact bytes to the
complete simulation configuration identity and the backend rechecks that
binding before use.
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
