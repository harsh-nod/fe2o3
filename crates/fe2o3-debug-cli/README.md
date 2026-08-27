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

Source operations require an authenticated exact-KIR source map, which this
command does not yet accept. Source variables, hardware registers, hardware
wave state, and KFD dispatch control are reported with typed `unavailable`
responses instead of fabricated values. Input KIR and request documents are
loaded through the same hardened parser and admission boundary as
`fe2o3-kir-sim`.
