# Typed Application Supervisor Handoff

Source: `b7b0c2e4081023d1e7dae684374c81cff724eb35`.

This completes the four-right transport and provisional supervisor-admission
prerequisite on the [multi-GPU critical path](../../runtime-multi-gpu-critical-path.md).
It does not complete authenticated root registration or an ordinary two-GPU application.

## Implementation

The distinct application launch owner duplicates both original pidfds captured
immediately after spawn. It does not reopen either process by numeric PID and
does not consume Cargo's original cleanup owner. The client independently
reconstructs the nested compiler handoff from original custody and the pinned
profile, then checks the fourth input against the actual prefork pair's recorded
child object and coordinate. One 840-byte seqpacket transfers exactly compiler
peer, app pidfd, proof peer and Cargo pidfd under one absolute deadline.

Supervisor admission uses a separate four-right receiver. It validates the exact
binding, Cargo control credentials, compiler peer, both original process identities
and direct parentage, descriptor roles, and the candidate proof peer's shape,
creator and continuity. Shared compiler admission retains the pre-receive control
snapshot. The existing ordinary 184-byte/two-right receiver remains strict.

Launch, pending and accepted application owners are move-only and cannot convert
to their compiler-only counterparts. They expose no application readiness or
launch API. Cargo and production listener dispatch deliberately remain unchanged
until the root registration/readiness gate can be enforced end to end.

Supervisor checks alone cannot authenticate the remote slot-4 counterpart: another
pair created by the same Cargo can pass this provisional admission. An explicit
test preserves that limit. Root observation must still prove exact reversed
addresses and the original remote slot object before granting readiness.

The client reuses runtime-protocol rather than duplicating its canonical codec.
This adds finalizer/Pliron/analysis dependencies to standalone client builds;
compiler-ffi was already transitive through artifact-transaction. The full static
deployment already includes runtime-protocol. This checkpoint does not newly
qualify deployed static images or measure standalone build cost.

## Qualification

| Check | Result |
| --- | --- |
| Broker unit tests | 207 passed; 12 default ignored |
| Client unit, binary and integration tests | 54 passed |
| Supervisor unit and integration tests | 61 passed; 4 default ignored |
| Cargo main, wrapper, proxy and fixture binaries | 444 passed; 5 default ignored |
| Broker, client and supervisor doc tests | 94 passed |
| Four-package all-target Clippy | Passed with `--no-deps -- -D warnings` |
| Scoped formatting, whitespace, frozen hashes and signed-source audit | Passed |

Total: 860 passed, excluding nested helper reruns. The 21 default-ignored entries
include subprocess helpers exercised by parent tests; root/deployment-only tests
were not separately enabled. New supervisor-positive tests use the private same-UID
test path, not a production cross-UID deployment. No application sandbox was widened.

Tests cover original-handle transfer without reopening, changed binding/pair,
closed control, expired deadline, child exit, wrong rights counts (including
truncation), swapped/aliased roles, corrupted bindings, wrong endpoint side,
changed proof flags, and compiler/application profile interchange. Compile-fail
tests cover cloning, descriptor extraction and ordinary-profile conversion.

## Evidence And Remaining Work

`evidence.tar.gz` contains final commands/logs, the source patch, 5,087 source/config
hashes, Cargo artifact records, 17 test-binary hashes, signed-source audit and an
internal manifest. Temporary local evidence staging was removed after verification;
the existing target cache was retained. No MI300X resources were created or changed.

Next: one atomic production dispatcher, root RegisterApplication/AttachApplication,
an independently bounded application table, authenticated app-first Hello and
challenge/accept/Ready exchange, and readiness gating outside the registry mutex.
Then deploy fixed keyless proof custody and join it to native invocation premises
before running the ordinary two-GPU fill and guarded bidirectional XGMI pipeline.
No new formal machine proof or performance claim is made here.

Source patch SHA256:
`496cd4f810e8d960e6fa59625f17ea687cff79bbb00856306190d2a2c5f38215`.

Archive SHA256:
`a0b3eb012ac05fadc662cbd92a0ac2fba76a93c94165e7826f1ddbcfa4be514f`.
