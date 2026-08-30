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

`fe2o3-debug sim --bundle-v2 KERNEL.fe2sim --request REQUEST.json` is a
separate, strict route for `VerifiedSimulationBundleV2`. The V2 envelope
contains one exact authority-free V1 execution bundle with no nested V1 map,
plus one canonical `fe2o3-debug-source-map-v2` payload. V1 decoders continue
to reject these bytes. V2 map records can name bounded lexical scopes, stable
variable identities, storage generations, and exact KIR SSA values at exact
block checkpoints. The production compiler exporter emits V1 by default and
can opt into this route with `--bundle-version 2`.
That route captures rustc lexical scopes in the same extraction session and
binds only exact one-to-one admitted kernel parameters whose entry value is
never moved, dropped, storage-reset, mutated, or mutably aliased in MIR to KIR
function values.
Other locals, projected/constant debug values, and non-one-to-one composite ABI
cases remain typed `Unrepresented`; no SSA lifetime is inferred. V1 remains
the byte-compatible default. Neither route authenticates compiler execution,
source refinement, or hardware behavior. The production regression is
`ordinary_kernel_sources_export_and_query_exact_v2_source_variables`.

Source-variable inspection uses the separate
`fe2o3-debug-source-variable-request-v2` schema. Callers select all variables,
one exact stable identity, or a bounded inert name. Name lookup chooses the
deepest active lexical scope and returns typed ambiguity when exact records do
not distinguish one binding. Values are existing typed simulator observations,
including allocation-relative pointers; native addresses and reconstructed
registers are never emitted. Missing V2 data, an empty V2 variable section,
out-of-scope names, unavailable frames/checkpoints, uninitialized generations,
lifetime kills, optimized-out values, unrepresented values, and truncated
captures remain distinct typed states. `all` queries use admitted per-function
indices and apply page bounds before value materialization.

`--replay-schedule SCHEDULE.json` securely admits the canonical persisted
semantic schedule and requires its exact raw-KIR/bundle route, KIR, complete
bundle identity and subject, request bytes, target, and limits to match this
debugger input before capture. The debugger configuration identity additionally
binds the schedule context, transcript, and exact decision-record integrity.
Protocol revisions still govern interactive navigation of the resulting
immutable transcript; they are not simulator schedule revisions. This is
deterministic CPU semantic replay, not GPU scheduling, timing, race-freedom,
equivalence, or performance evidence.

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

`fe2o3-debug-diagnosis-request-v2` adds a read-only, page-bounded diagnosis
query to the same simulator JSONL stream without changing the closed V1
operation set. For a retained out-of-bounds failure it returns the exact
trace-local backing allocation, requested range, and narrower legal pointer or
slice view plus the admitted allocation and uniquely joined kernel-ABI
argument contract. Aliased views retain both their common backing identity and
their distinct ABI ordinal/range, so an access can be outside one view while
remaining inside the backing allocation. ABI-required access, request
buffer/view access, and backing-allocation access remain distinct and are
joined with the simulator's monotonic capability rule. For retained
workgroup-barrier divergence it returns the exact phase, declared scope,
ordering and address-space semantics, a phase-derived LDS epoch, complete
arrived/waiting/exited local participant inventories, the observed arrival
count when the transcript is complete, and the expected participant inventory
derived from admitted launch geometry. An LDS epoch after a barrier that did
not release remains unavailable.

Every diagnosis also binds the stable simulator configuration, a
domain-separated identity of the exact request and canonical KIR dispatch
input, and the exact KIR operation when retained. A Source Map V2 can add the
exact source operation and span with its existing caller- or bundle-bound
provenance. Verified bundle envelopes add exact envelope, inner subject,
production-KIR, ABI, and source-lineage receipt references. Those references
are content association only: the debugger does not relabel lineage receipts
as property proofs, and finalized artifact/property fields remain typed
unavailable without independent authority.

Each material fact carries a claim identity tied to a retained input,
simulator-terminal, simulator-transcript, KIR-operation, ABI, source-map,
derived, or availability record identity. The evidence manifest also binds
session revision, configuration, completeness, finding sequence/class, exact
admitted input, and canonical bounded terminal/transcript content. Hashes and
wire decoding prove content integrity, not producer identity. Before returning
a production Bundle V2 diagnosis, the CLI independently rederives the expected
capture binding from its owned deterministic simulator result and exact Bundle
envelope/subject, full input manifest, session/completeness, and response
wrapper (request, operation, and pagination cursor), then exact-compares it
with the response. Source spans include a bounded membership proof against the
exact admitted Source Map V2 operation inventory; changing a span to another
range in the same map fails validation.

Dispatch geometry is `declared`; terminal invocation, KIR site, dynamic range,
phase, and arrived/waiting/exited local participants are CPU-semantic
`observed` facts; expected locals, global participant coordinates, logical
wave/lane partitions, logical element indices, legal-bounds results, and the
current LDS epoch are `inferred` with their derivation; absent facts remain
typed `unavailable`. This diagnoses any
admitted KIR kernel that reaches those simulator error classes and contains no
kernel-name or fixture-specific rule.

The diagnosis schema validates only CPU-simulator sessions. Live KFD and
ROCgdb keep their own separately versioned capability contracts: stopped wave,
lane, register, PC, source, and target-memory facts remain unsupported or not
captured there. A simulator diagnosis may be correlated by an agent as
reference evidence, but it is never upgraded to native hardware state or used
as evidence that the GPU executed the same path.

Barrier residency is replayed from semantic records. A lane is
`barrier_blocked` from its arrival through the record before the matching
workgroup release. A wave or workgroup is `barrier_blocked` only when every
active lane in that aggregate is waiting; a partial wait is `runnable`.
Dispatch aggregation follows the currently scheduled workgroup. The release
record clears residency before scope state is reported, so its representative
lane is `running` and other released lanes are `runnable` until their next
record.

`--source-map MAP --source-bundle-subject SUBJECT` admits a strict, bounded
canonical `fe2o3-debug-source-map-v1` or `fe2o3-debug-source-map-v2` sidecar.
Both options are required together. The map binds the canonical KIR digest and length plus a non-circular
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
provenance. A loose V2 map can resolve diagnosis source operations and V2
source-variable records, but its provenance is still caller-bound. Production compiler integration must call
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
Frames are not reconstructed from names or UI fixtures. Source variables are
available only through the exact V2 bundle route above; V1 and hardware V2
sessions keep them typed unavailable. Hardware registers, hardware wave state,
and KFD dispatch control remain typed `unavailable`; no value is fabricated.
KIR, request, and sidecar files use the hardened regular-file capture boundary
shared with `fe2o3-kir-sim`.

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

## Exact-bound live KFD protocol V3

`fe2o3-debug live-kfd --bundle-v2 KERNEL.fe2sim --request REQUEST.json
--hsaco KERNEL.hsaco -- PROGRAM [ARG...]` composes the deterministic CPU
reference inputs with the direct-KFD lifecycle without conflating their truth
classes. Admission captures and revalidates four distinct regular files,
rejects symlinks, hard links, role aliases, changing inputs, invalid HSACO, and
an unexecutable host. The coordinator launches through the retained executable
descriptor and upgrades that content binding only after its owned exec stop.
It never claims that declared HSACO bytes were loaded or executed.

The target may call
`admit_inherited_kfd_target_debug_telemetry_v1` and send fixed-size cooperative
records. The inherited endpoint is private, send-only, credential-bound, and
contains no KFD authority, native identifier, pointer, address, path, or file
descriptor. Target artifact, dispatch, allocation, and diagnostic facts remain
labeled `declared`. Independently observed devices, queues, runtime events, and
queue-control effects remain KFD observations. A matching target code-object
declaration does not upgrade `execution_code_object`, which stays typed
`not_observed` until an independent execution observation exists.

V3 exposes exact bundle/request/KIR/source-map/HSACO correlation, bounded
target-telemetry summaries, redacted KFD device and queue snapshots, exception
events, queue suspend/resume, and termination over one agent-friendly JSONL
stream. After a successful `suspend_queues`, a client can capture the exact
session-owned queue envelope without granting new authority:

```json
{"schema":"fe2o3-live-gpu-debug-request-v3","operation":"capture_stopped_queue_envelope","request_id":3,"expected_revision":1,"queue":{"generation":1,"ordinal":1}}
```

The response deliberately keeps the overall session `running`; only the named
logical queue has retained session-owned suspension. It contains address-free
queue/device/save-area/header identities, exception bits, ring and queue shape,
gfx target, XCC count, relative header ranges, and error-binding presence. It
contains no PID, native queue/device ID, descriptor, virtual address, PC,
register, checkpoint bytes, source path, or target-memory value. The response
identities use a fresh private random per-session correlation scope, not the
public artifact binding, so repeated sessions are not intentionally linkable.
The scope itself is never serialized or logged. The response
sets `resume_required: true`; capture never resumes the queue, and the existing
revision-checked `resume_queues` operation is required. Suspension ownership is
bound to the exact queue and device snapshot. Native-ID reuse or binding
substitution invalidates the logical generation, terminally poisons the public
session, and immediately enters the existing KFD session-finish cleanup; the
facade never discards suspension ownership while the KFD engine still owns it.

The CPU-visible context header envelope is a bounded, sequential, non-atomic
observation: KFD queue/device snapshots and the eight XCC VMA header reads occur
in order with binding checks before and after; they are not one simultaneous
hardware checkpoint. The envelope is not decoded hardware checkpoint state.
Hardware checkpoint bytes, wave records, lane state, registers, PC, source,
and memory remain separately typed unavailable with exact KFD reasons.
The generic stopped anchor remains `session_not_stopped`, and stopped
dispatch/workgroup/wave/lane queries remain `unsupported`. The MI300X
live-validation test creates a real target KFD queue, observes it, suspends it,
captures the bounded envelope, explicitly resumes it, and terminates cleanly;
it deliberately does not load or execute its declared fixture HSACO.

## Structured live ROCgdb protocol V3

`fe2o3-debug live-rocgdb --rocgdb /usr/bin/rocgdb --authorization ID --
PROGRAM [ARG...]` launches the exact argument vector through ROCgdb's GDB/MI3
interpreter. `--attach PID` selects an authorized attach instead. The PID,
ROCgdb path, target path and arguments are bootstrap authorities and never
appear in JSONL responses. The coordinator invokes no shell and uses no HIP or
HSA runtime.

The `fe2o3-rocgdb-mi-request-v3` JSONL protocol exposes structured capability
discovery, asynchronous events, caller-selected generic thread admission from
`-thread-info` tuple ordinals, and audited breakpoint/continue/pause/step
control. Native code-object and allocation addresses and source paths occur
only in admission requests; their responses contain content, allocation, or
source-span identities. Request lines, response lines, command counts, MI
records, nesting, strings and waits are bounded. Duplicate request IDs, stale
revisions, authorization mismatch, unknown fields, aliases and malformed MI
are rejected without screen scraping.

An admitted MI thread remains a generic logical debugger thread. Host, unknown,
and GPU-looking `target-id` text is not GPU classification evidence. Stopped
dispatch/workgroup/wave/lane, relative PC/source, register/value/expression,
and allocation-relative memory capabilities remain typed `unsupported` until a
separate trusted correlation source can authenticate a GPU thread binding; no
wave or topology coordinate is synthesized. ROCgdb and direct-KFD debug-trap
control are mutually selected session backends, although direct-KFD runtime
telemetry may still be correlated when it does not claim a second debug-trap
owner.

The deterministic MI fixture validates the currently available JSONL workflow
in generic CI. Installed ROCgdb capability discovery is validated on MI300X. A
live pure-KFD GPU wave stop with PC, `exec`, registers and resume has not yet
been validated, so this command does not claim that milestone.
