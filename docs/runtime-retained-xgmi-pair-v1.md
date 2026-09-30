# Ordinary-Lifetime Retained XGMI Pair

This experimental opt-in profile is separate from the existing full-fresh XGMI
queue and batch APIs. It does not change their observation requirements or
receipts, and is never selected as a fallback after a failed full-fresh check.

`begin_ordinary_retained_pair_v1` requires an explicit
`Gfx942XgmiRetainedPairEnvironmentAssumptionV1`. Its single variant acknowledges
the reviewed MI300X/Linux 6.8.0-124/AMDGPU DKMS 6.16.13 ordinary-lifetime contract.
It is not a runtime check of the loaded module, an attestation, or resource
authority. A native qualification must independently record the real kernel,
module/build ID and package/source identities before using this assumption.

The caller's environment must exclude **all** administrative repartition, hive
reconfiguration, hotplug, privileged CRIU, and foreign same-process raw KFD/DRM
mutation while the scope is active. Some operations are blocked by the reviewed
driver while live process-device owners exist, but that is not a universal
administrative-interference guarantee. Other Linux/gfx942 installations are not
implicitly covered merely because the device target matches.

## Held Objects

Entry checks a live, drained directional queue with vacant retirement custody,
all four native record rosters empty, and no uncertain ticket. Its ring, control
and completion allocations are authenticated against the exact source session
and VM. The directional source/peer pair and retained peer domain are checked,
then the existing full-pair observation runs once.

The new scope exclusively borrows the actual queue and both memory sessions.
Each submitted request still moves its exact source and destination mappings
into existing queue records. Private mapping identities, ranges, overlap,
capacity, packet construction, queue occurrence and ticket generation checks
remain in the existing implementation. The profile neither caches raw data
addresses as authority nor releases data mappings while a ticket is pending.

The reviewed driver's PDD retains its acquired DRM file/VM. Peer attachments
retain GEM BO references and mapped allocations cannot be freed. Queue buffer
references protect queue controls, not arbitrary packet data, so fe2o3's moved
mapping custody remains essential. Eviction can change physical placement;
kernel-managed restoration updates PTEs before restoring queues. None of these
contracts pins physical link availability or makes reset transparent.

## Results And Failure

Every publication and completion is bracketed by both endpoint operational
checks: opener PID, non-draining reset readiness, the admitted wrapping DRM
VRAM-loss counter, and closing reset readiness. Exact retained pair binding
precedes those live observations. No full sysfs discovery occurs per operation.

A retained-profile success is returned only after its own closing checks pass.
Its distinct result wrapper never converts to an old full-fresh completion or
currentness diagnostic. Scope finish is not a delayed authority witness for
earlier successful operations; it checks drained/live state and operational
currentness only. Later faults cannot retroactively invalidate a past hardware
completion, and a prior completion cannot rehabilitate a terminal session.

Successfully fenced timeouts keep the exact tickets and queue-owned mappings
and may be retried. Currentness/native uncertainty or panic eagerly quarantines
both sessions and poisons the queue and process gate before return/unwind.
Even an unexpected internal success with terminal custody becomes a failure
retaining its tickets or indeterminate mappings. No recovery accessor presents
indeterminate mappings as a full-fresh or retained successful completion.

Consuming finish with pending work fails closed. Dropping an unfinished scope
quarantines it. `mem::forget` skips Drop, but cannot release queue-owned pending
records, undo eager terminalization, or bypass a check needed by a prior success:
each such success already completed its own operational closing checks.

Reset subscription gaps, wrapping counters, kernel/firmware correctness and
unreported reset classes remain external limits, as documented for existing
retained queues. This is not fresh whole-host evidence, universal reset/hotplug
coverage, a kernel/ISA proof, or a performance result.

## Qualification Boundary

The lifecycle controller has a shared executable body for subsequent source
refinement. CPU fixture tests do not grant native authority or prove the driver.
The native API and runtime facade are distinct timing surfaces: a native retained
benchmark must identify that surface and this profile explicitly, not reuse the
historical facade/full-fresh labels. Operational-fence attribution belongs in a
separate timing population. Matched HIP/HSA comparisons require new qualification
and measurement; no gain or parity is implied by this design.

## Runtime Context Facade

`RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>::begin_retained_peer_copy_batch_v1`
borrows the Context and the complete mutable pending ordinary peer-copy roster.
This first profile accepts 1..=63 scalar, disjoint copies in one direction. It
excludes other pending work, events, callbacks, dependencies, directed copies,
segmented copies and generated launches. Idle streams may remain present.
It is a concrete backend API, not a generic provider or new native authority.

Entry prepares and retains the actual queue/session owners and fixes an absolute
deadline that includes preparation. It does not publish packets. The first
`wait` publishes once and waits against that deadline; later waits observe the
same tickets without republishing or extending the deadline. A fenced timeout
returns `Pending` while retaining custody. `ReadyToFinish` is distinct from
ordinary submission success. Consuming `finish` restores the native owners and
then commits logical Context completion. Finishing while pending is a terminal
refusal, not cancellation.

Dropping an unfinished facade quarantines the Context and backend. Forgetting it
leaves a persistent Context reservation that blocks ordinary work and cleanup.
Lossless timeout recovery preserves the entire failure on an unexpected custody
variant. These safeguards do not create an administrative-reset guarantee or a
full-fresh completion witness. CPU storage/driver/Context fixtures and source
wiring tests are not execution of this public facade on two live GPUs.

Signed facade candidate `b1ad04dc7`, integrated at `304042041`, is bound to
accepted pre-signing development checks: 582 distinct runtime CPU tests, all
31 retained-pair KFD tests, all 55 runtime doctests and two source-only
integration tests. Strict all-feature/all-target runtime Clippy, warning-free
no-default compilation and scoped static checks pass. Runtime test execution
uses `cpu-runtime-fixtures,hardware-diagnostic`; all-feature static analysis is
not all-feature execution. The KFD executable is explicitly cache-qualified.
All 6446 selected candidate inputs match signed source, and all eighteen fresh
same-namespace process groups close. Rejected earlier attempts remain preserved.
These local records are not a signed-source rerun, full-runtime acceptance or
a formal proof of this facade. Refreshed complete-tree proof guards preserve
their unchanged executable proof closures and do not extend those theorems to
the facade.

The runtime peer benchmark accepts the separate
`--retained-peer-batch-reviewed-mi300x` flag. Its retained mode permits depths
1..=32, nonzero samples and at most 10000 warmup plus sample rounds, checked before
device opening. It emits `fe2o3.xgmi-peer-retained-facade-benchmark.v1`, never an
ordinary/full-fresh or native-series row. Headline timing includes facade enqueue,
scope entry, first publication/wait, consuming finish and logical settlement.
Handle release is outside timing and labeled accordingly. There is one scope per
batch, not one scope spanning the full sample series.

Both directions have a prime batch and alternate during the hot sequence, with
no intervening host read/write. Final readback checks source and destination
payloads and canaries; reused payload bytes do not independently prove fresh
work at every iteration. Rows include the complete sorted sample populations,
explicit profile/policy/environment-assumption labels and appear only after
successful validation and explicit native teardown. The existing parity checker
rejects this new schema. Merged-tree benchmark unit tests pass 18/18 with no
default features and 19/19 with all features; all 29 parity-report tests and
strict all-feature/all-target Clippy pass. These are local development results,
not hardware qualification. A reviewed native runner, observed driver/environment
binding and matched HIP/HSA qualification remain required; no measured speedup
or hardware acceptance is claimed.

## Directional-Series Benchmark

The native KFD example accepts the explicit
`--retained-pair-series-reviewed-mi300x` opt-in after its existing six controls.
That flag acknowledges the environment assumption above; it does not detect the
driver. HIP and HSA comparators accept `--persistent-series`. Their historical
ordinary, alternating persistent-hot and ordered-segment modes remain separate.

All three series producers prepare both directions before copying, execute one
prime and the requested warmup/sample batches in a complete forward series,
then do the same in reverse. They use the same per-slot patterns, payload size,
32-byte prefix/suffix canaries and final readback population. No host write or
readback interrupts either copy series. Reusing identical bytes means final
data validation does **not** independently prove fresh bytes for every timed
iteration; native completion/ticket semantics provide that progress boundary.

KFD holds one directional scope through that direction's entire series. Its
request preparation and completed-mapping extraction are outside the sample;
native submit through observed completion, including both endpoint opening and
closing operational fences, is inside. Full initial scope admission and final
drained closure are outside samples and reported separately as entry/finish
durations. Those durations are not operational-fence attribution samples. No
separate attribution instrumentation is mixed into this ordinary population.
HIP/HSA timing still includes their native enqueue and completion wait/reset.
Engine selection/parallelism remains explicitly different, not assumed equal.

`xgmi_peer_series_results.py` requires new schemas, exact physical unique IDs,
trial controls, sample counts, final-validation scope, and the KFD profile,
policy digest and environment-assumption label. KFD's numeric GPU IDs and
directional engine IDs must also match independent trial inputs. The old hot
parser rejects these rows, and the new parser rejects old rows. A matching row
does not authenticate a driver or create native evidence. The runtime-facade
integration above has a separate end-to-end timing surface; native-series rows
cannot qualify its performance.

`xgmi_peer_series_campaign.py` is a pure plan/replay module, not a native runner.
It fixes depths 1/16/32 and the backend order KFD, HSA, HIP, HIP, HSA, KFD at each
depth, producing 18 separately recorded trials. Default controls are 1 MiB per
copy, two warmups and ten samples per direction. Its input joins selected
physical indices and canonical UIDs to BDFs, KFD GPU IDs, directional KFD route
engines, and independently observed HIP/HSA filtered indices 0/1. KFD's chosen
engine is not imputed to HIP or HSA; both comparators retain their
runtime-selected-unknown routing label.

The module requires before/after observations of the reviewed kernel/module,
package and 22 selected source hashes, with explicit ordinary-lifetime
assumptions. Those field/digest joins do not authenticate the observations or
prove that source compiled into the module. It checks every ordered command and
its strictly parsed output, including actual forward/reverse sample-vector
counts, final canary/readback success and explicit producer teardown. Separate
execution-receipt references must be distinct. A caller must still authenticate
each referenced raw receipt, bind signed source and actual ELF bytes, enforce
shared-host admission/postflight and deadlines, collect all artifacts before
owned cleanup, and independently establish process/path absence. A consistency
replay alone is not native acceptance or interruption-safety qualification.
