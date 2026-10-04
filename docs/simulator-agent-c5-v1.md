# Simulator diagnosis agent V1

`fe2o3-sim-agent` is an experimental, fresh-process JSONL service for bounded,
read-only diagnosis of two retained CPU-simulation failures:

- an exact data race paired with a canonical
  `SimulationFailureReductionReportV1`; and
- a host operation attempted while a virtual-runtime buffer is retained by one
  or more prepared or ambiguous dispatches.

It accepts evidence on stdin and emits one canonical response per line on
stdout. It has no operation for simulation, compilation, hardware execution,
file or network access, source editing, or patch application. All command-line
arguments are rejected. `discover_capabilities` reports this authority boundary
and the exact limits: 128 requests per process, a 20 MiB request line, a 4 MiB
response line, 8 MiB of decoded input evidence, and 256 items per page. Host
lifetime capture separately caps canonical evidence at 4 MiB, retained blockers
at 256, and dispatch-input snapshot hashing at 64 MiB per incident.

## Evidence admission

`open_race` takes the canonical reduction-report bytes as lowercase hex, the
detailed first race, and caller-pinned KIR, context, and report identities. The
service verifies the report codec and identities, verifies that the detailed
race matches the report's exact race fingerprint, and rejects malformed race
geometry, invalid semantic sites, or substituted artifacts. A report may name
either the canonical schedule or an exact deterministic seed.

`open_host_lifetime` takes canonical
`VirtualHostLifetimeEvidenceV1` bytes as lowercase hex plus caller-pinned
runtime and incident identities. The evidence is produced by the read-only
`VirtualRuntimeV1::capture_host_lifetime_evidence_v1` query. It binds the
runtime generation, buffer ordinal and access, attempted operation, retained
completion/queue/module identities, exact KIR identity, completion state, and,
when the byte budget permits, an exact dispatch-input snapshot identity.

The host evidence codec rejects unknown fields, non-canonical bytes, duplicate
blockers, inconsistent retained counts, identity changes, and completeness
upgrades. Exhausting the blocker or input-snapshot budget produces a typed
partial state; it never substitutes a local ordinal or claims completeness.
The content identities provide integrity and correlation, not producer
authentication.

## Diagnosis and reduction

After one immutable evidence set is open, `diagnose` returns:

- for a race, the two exact invocations and semantic sites, allocation and byte
  range, access and atomic properties, schedule kind, reduction coverage, and
  all report/reproducer/race identities; or
- for host-lifetime misuse, the declared attempted operation and a finding
  inferred from observed virtual state, with the exact retained blockers and
  incident completeness.

Race output describes one observed pair of unordered conflicting accesses. It
does not prove a modeled happens-before relation, exhaustive schedule coverage,
race freedom, producer authenticity, or GPU behavior. Those unavailable facts
are typed in the response.

`reduce` only pages a witness already present in the admitted evidence. Race
pages expose the simulator-verified reproducer decisions, each with a
content-bound identity. A locally minimal report is labeled as such; otherwise
the missing local-minimum claim is typed. Host pages retain one positive
blocking completion, which is sufficient to demonstrate that the observed
buffer is retained. This is not a reduction of runtime history and does not
establish that other blockers do not exist when the incident is partial.

Every cursor binds the session, source evidence, and next offset. Page limits
are 1 through 256 items. Reusing a cursor with another session/evidence set,
skipping an offset, replacing an already-open evidence set, reusing a request
ID, or sending a stale revision is rejected. A session terminates explicitly or
after 128 requests; there is no implicit state reuse between processes.

## Compatibility and limits

This workflow adds new evidence and service schemas. Existing simulator,
reduction-report, debugger, and virtual-runtime V1 wire formats are unchanged.
The current service handles canonical simulator races, deterministic seeded
simulator races, and retained-buffer host-lifetime incidents across admitted
kernel and request shapes. It does not predict CPU or GPU performance, execute
a proposed reduced input, minimize kernel data, diagnose unobserved races, or
grant compiler, artifact, launch, hardware, or correctness authority.
