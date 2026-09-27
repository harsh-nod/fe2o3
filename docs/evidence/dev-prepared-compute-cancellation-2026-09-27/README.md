# Prepared Compute Cancellation Custody

CPU developer evidence, not GPU execution, executable formal refinement,
complete #182/A1/A2 closure, HIP/HSA parity, or a performance comparison.

Baseline: `0d7c4a59f26ba0462c51411089de4daf017879fc`.
Signed implementation: `da646078e23db6546cdb72fea2730adcfb97b008`.
Signed final source, including strengthened assertions:
`e55af4cae1be9ee67c962706cd2ee933aa550a97`.
Source tree: `e46d357c3b01897d888b34445df74921b9d0798d`.

## Change

Prepared cancellation previously removed the outer active descriptor before
native cancellation and runtime restoration. A new public-admission regression
reproduced its loss after private mutation of an allocation's ComputeInFlight
marker. This is fault injection, not an observed GPU failure.

The descriptor now remains indexed through an explicit cancellation phase.
Single/three native receipts, returned typed inputs, admissions, promotions,
restore shells and publication provenance have a retained cancellation root.
A native terminal result leaves its receipt or native custody in the root,
terminal custody slot, or lower queue as appropriate. Unwind poisons the backend
and preserves the original panic payload without dropping the outer descriptor.

Cancellation checks local exact Compute memberships, distinct allocation roster,
kernel/module/stream binding, module retain, lane lease, completion reservation,
result-key vacancy, retained-control provenance and restoration prerequisites.
Existing private monotone owner indexes remain a premise; this is not a global
conservation proof or arbitrary metadata-corruption detector.

The result-map reserve, single-input restoration shells and boxed cancellation
root are reserved before any custody movement or native cancellation effect.
The box avoids adding three inline input owners to every published pipeline
slot. Returned inputs are indexed before restoration. All restoration slots are
checked before the first write; each allocation-free, callback-free restore
records its completed prefix before any test fault boundary. Logical release
removes active last. Stream-tail selection explicitly excludes the cancelled ID.

Exact retained control is authenticated using the admission's recorded reuse
flag, not merely its input source: initialized storage may reuse matching
control after replay. The original single/three restoration matching and writes
now share shell helpers. Checks and restoration visit at most three bindings;
existing custody release costs are unchanged. No latency improvement is claimed.

## Tests

The new scripted three-binding Prepared state retains the original typed inputs
and preallocated shells before normalization/publication. Polling can still
publish it; cancellation does not fabricate a dispatch-publication event.

Forty-four subprocess cases cover sixteen preflight mutations and six consuming
or restoration-prefix failures for each of one and three initialized-storage
inputs. They check exact native-owner identities, byte backing addresses and
digests, original or consumed restore-shell addresses, publication provenance,
outer metadata, retain/reservation accounting, events, stream tails and lane
leases. They also assert the original Prepared phase and unchanged markers on
preflight rejection, and exact cancellation phase/slot occupancy plus unrestored
markers after consuming/prefix failures. Repeated terminal ingress cannot release
the owners. Every child then drops the unrepaired backend and must SIGABRT after
an inspection marker, with core dumps disabled.

Eight healthy cases cover single/three-input cancellation from seeded H2D,
initialized, replay and initialized-after-replay storage, preserving exact typed
owners and clearing only completed cancellation custody. Four three-binding
polling controls cover the same origins. The generic H2D fixture is seeded test
custody, not an upload witness. Existing public write/copy/poll single-input H2D
cancellation, profiler, released-stream-tail and exact-control replay tests are
also included in the regression suite. Healthy scripted cleanup disables device
scrubbing; no hardware scrub claim follows.

## Qualification

The final signed full all-feature runtime/model command returned 101:

- Runtime unit tests: 1,733 passed, three failed, 28 ignored.
- Runtime-model unit tests: 1,080 passed, 19 ignored.
- Integration tests: 11 passed, three hardware tests ignored.
- Runtime doctests: 52 passed (eight plus 44); model doctests: 29 passed.

The three failures remain explicit: `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. Each fails with
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`, as in the
preceding checkpoint. The full suite is not green and telemetry is not waived.

Strict all-feature/all-target runtime/model Clippy passed after the strengthened
assertions. The no-default-feature runtime check passed. The focused runtime KFD
suite passed 836 tests, with 28 ignored, before the final test-only strengthening;
the final full run includes these tests. Source signatures and source continuity
passed. Commands, logs and exit statuses are preserved in `receipts.tar.xz`, with
its companion SHA-256 file identifying the frozen bytes.

The separate all-feature KFD `persistent_cancel` selection passed 17 tests, with
1,666 filtered out and none ignored. This includes constructed lower cleanup,
restoration-prefix, panic-payload and initialization-origin cases. It is CPU
lower-body evidence, not a runtime-to-GPU cancellation witness.

## Open Boundaries

The scripted terminal/unwind seam runs with returned inputs retained. It does
not directly exercise the outer native Single Retryable/ProcessTeardown or
Three recovered/queue-owned return mapping. Review found those branches retain
custody, but native coupled runtime qualification remains open. Separate lower
KFD constructed-cancellation tests do not establish end-to-end correspondence.

Failure injection uses initialized-storage inputs; H2D and replay have positive
coverage here. Allocator-exhaustion injection, full resource conservation,
Context composition, every consuming publication/observation path, executable
formal refinement and matched HIP/HSA hardware benchmarks remain open. Restored
prefixes rely on the private no-reentry, callback-free transition, not a theorem
covering arbitrary concurrent corruption.

The MI300X probe failed DNS resolution for `sharkmi300x-1`; no remote files or
jobs were created. This checkpoint neither establishes hardware parity nor
closes the broad runtime roadmap.
