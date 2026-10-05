# Original Native Attempt Integration

The private compiler Attempt now retains either its gated original trace or the
existing original-trace issuer attempt. The executable inventory and controller
stay with that same outer owner. There is no independently supplied runtime,
PID-based trace reconstruction, provider guard, or second tracing pipeline.

The implemented transitions are original gate interrupt, consuming interrupt
observation, original runtime takeover, non-resuming first-exec image/census
capture, alias closure followed by native exec EOF and confirmation, existing
issuer launch/readiness, and original controller stepping. Typed checkpoint and
output-confinement modes are both required by final staging validation. The
unchanged original deadline is retained throughout; issuer launch receives only
its remaining duration.

First-exec confirmation closes the complete Stage and both retained gate aliases
before reading the original status pipe. It requires actual clean EOF, held
image validation, and the original trace's exec/channel checks. Issuer transfer
is consuming and cannot substitute another trace. The first checkpoint is not
reachable through this owner until actual issuer readiness has passed.

Checkpoint access keeps the ready helper's original locked resources, runtime
identity, live process/profile, and original account. Full content validation
occurs at ownership transitions; immutable executable metadata and actual file
identities/ranges are checked at checkpoints without hashing the whole compiler
for every syscall. Immutability and exclusion of external privileged mutation
remain custody requirements, not conclusions from cached metadata.

Cancellation retains runtime descendants for funded foreground retirement.
Trace retirement, issuer cleanup, publication observation, and aggregate pool
emptiness are distinct. A consuming transition that cannot preserve unresolved
runtime custody must fail-stop under the dedicated-process contract.

This patch does not open the compiler gate or change the production request's
refusal selection. Device confinement must be authenticated through the actual
original cgroup observation before a separate gate-release integration. Ordinary
compilation, exact/one-short joined controller accounting, actual first-exec/EOF
ordering, issuer readiness and end-to-end native compilation still require
qualification; source and mechanical quote tests do not establish those results.
