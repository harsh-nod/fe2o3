# fe2o3 compiler driver

`fe2o3-compiler-driver` owns explicit routing for the two
`PipelineSelectorV1` values. Each request invokes exactly one configured
backend. A failed or malformed backend transaction is converted to a bounded,
canonical rejected output; the driver never attempts another selector.

Backend outputs are revalidated against the actual request before they cross
the driver boundary. This preserves receipt and obligation chains and prevents
an output validated for one request from being replayed for another.
`PlironShadow` is inspect-only, and any executable candidate returned from its
slot is rejected before output revalidation. `PlironV1` is the sole
candidate-producing route.

## Proof-required tiled GEMM

The issue #138 compiler lane adds a GEMM-specific gate in front of candidate
construction. It names twelve independent safety and refinement properties,
requires one discharged aggregate result for each, and separately requires
every obligation derived from unsafe source behavior to be discharged. Missing,
duplicate, unsupported, timed-out, incomplete, or counterexample results return
a stable property diagnostic and a transactional rejection with no stage
snapshot, object, HSACO candidate, or call into the admitted backend.

The report is bound to the compile request's exact obligation-set commitment.
Separately, the compiler supplies a request-bound expected unsafe inventory
derived from authenticated MIR. Reported unsafe findings must match that
inventory one-for-one by stable obligation ID, property, and retained semantic
subject. Missing, unexpected, duplicated, or substituted findings reject, so a
report provider cannot gain admission by omitting unsafe behavior. Unsafe
findings supplement the required aggregate properties and cannot replace them.
A private compiler-local admission token is consumed by the only backend trait
that can begin candidate construction, so writing `unsafe` never creates proof
authority or bypasses the proof-required gate.

A later adapter from the accepted `kernel.*`, `schedule.*`, `tile.*`, and
`gpu.*` dialect operations will derive the report and complete unsafe inventory
from authenticated compiler state. The current report is an untrusted bounded
contract; a report value, dialect verifier result, or admission token is not
proof evidence and does not authenticate how an obligation was discharged.

## Authority boundary

This crate routes in-memory compiler API contracts. It does not invoke COMGR,
publish artifacts, read or write artifact stores, load modules, dispatch work,
or launch kernels. An output candidate remains opaque and grants none of those
authorities.
