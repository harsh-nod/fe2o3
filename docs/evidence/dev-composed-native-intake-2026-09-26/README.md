# Lower Composed Native Intake

Base: signed `f091df84304b7c8045529680ab04a87955b99691`.
This packet connects typed request/N1/N2 admission to lower KFD memory and
compute-queue constructors. It does not enable the composed Runtime/Context
profile or close MEM-DOM, A1/A2, formal refinement or HIP/HSA parity.

## Contract

The new composed memory and compute-queue constructors consume the entire
move-only admission. Exact checked-device binding and observed request/session
health are checked before allocating the VM identity or entering the process VM
attempt. After VM acquisition, the installer checks private bundle coherence,
device generation, VM binding and pristine native-account configuration. It
constructs both native adapters locally, validates currentness and rechecks
session health before committing request, host and device accounts together.
There is no fallible tail between those field assignments.

Healthy reserved and retained request charges are admissible: a future Context
must retain its request before native startup. Configuration failure leaves all
three fields unchanged. Currentness failure seals the engine; installation
unwind seals it and resumes the original panic. Queue-owned and loaned engines
cannot reopen configuration. Native-only budgets, defaults and validation order
remain unchanged through the shared two-adapter installation helper.

The engine holds a typed request account after native allocation/account fields
and observes inclusive session usage in its phase check. Request quarantine
therefore seals checked allocation boundaries of the affected session, while
an unrelated healthy session can still use spare root/device capacity. A health
snapshot is not atomic exclusion against concurrent request-clone quarantine
during an already-started operation. No universal instant-of-quarantine theorem
is claimed.

Logical requested bytes, padded host backing and padded device backing retain
separate charges under shared ancestors. Releasing one does not refund another.
Queue-foundation loans and pool retags preserve native charges. Inclusive
`native_backing_usage_v1` includes requests for this profile; it is not a
native-only residency reading or a logical-allocation count. Internal queue and
bootstrap backing do not synthesize logical request charges.

## Verification

The initial focused selection passed 25 tests. The expanded pre-mutation
selection passed 27: fourteen typed-account/minting groups, eleven composed
native-adapter groups, the existing foundation/pool-retag group extended to this
profile in both startup orders, and the constructor source-routing group.
Selections overlap broader library coverage and are not additive coverage.

New tests cover healthy reserved/retained intake; all six class orderings;
independent refunds; combined root/device pressure across sessions; wrong
generation/VM and existing/closed/history configurations; currentness error and
original panic; preexisting and during-install quarantine; selected-session
sealing without poisoning siblings; clean failed-install reclamation; request
tokens outliving a clean engine; native raw-drop quarantine and registry custody;
queue-owned/loaned rejection; and host/device disposal errors and panics at every
tested boundary. Private substitution tests reject mixed roots/sessions at
actual admission splitting. Raw-drop tests do not independently prove request
field ordering because native charges retain their own typed root bindings.

FakeBackend exercises production accounting adapters and ownership transitions,
not Linux ioctls, real checked-device acquisition, physical pool reuse or native
shutdown. Constructor ordering checks are source assertions, not execution of
the Linux constructor.

The deliberate Rust mutation removes the post-currentness session-health check.
The selected test compiles and fails its uninstalled-account assertion (exit
101), detecting publication of the request account after injected quarantine.
The mutation patch, source digests and log are retained; the original source
digest was restored before the final validation selection, which again passes
all 27 focused tests. This is a regression control, not a formal proof.

Broad libraries report 1,034 model, 70 accounting, 1,346 KFD and 1,482 runtime
passes (3,932 total), with the same four socket failures: KFD
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
at `SocketAdmission`, and Runtime
`authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`authorized_execution::tests::failed_session_end_is_explicit_and_terminal`, and
`authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned`
at `InspectSocket(EPERM)`. Library exit is 101. The broad run filters 296 KFD
construction cases and preserves 19 model/28 Runtime ignores. It is not full CPU
qualification, and ignored native tests are not passes.

All 69 selected construction/custody tests pass (67 KFD, two Runtime), as do
107 doctests (31 KFD, three accounting, 46 Runtime, 27 model), strict
all-feature/all-target Clippy and no-default-feature checking. These selections
overlap the library coverage. Workspace formatting also passes. The validation
driver is terminal with exit 1 because libraries returned 101; every other stage
returned zero.

The four package file lists and source digests plus workspace Cargo inputs match
before/after validation and before commit. The four actual library test ELF
executables have recorded SHA-256 digests, rechecked before cleanup. Only the
owned 709,740 KiB build target was removed after the driver became terminal,
followed by a separate successful absence check. Receipts are under `raw/`.

The prior accounting/model/proof source manifest rechecks unchanged. No new
Verus execution or proof of the adapter, mutex, minting, native correspondence
or full composition is claimed. Earlier pure planner/arena proofs do not cover
this new installation boundary.

## Remaining Work

Runtime must adopt the exact request leaf without adding a fifth hierarchy level,
install complete device-bound account rosters at Context open, and carry exact
borrowed request witnesses before backend/native effects. Its direct allocation
entry points and returned terminal/shutdown backends must not bypass witnesses.
Generated rosters need witness-bound plans and atomic retain/metadata adoption.
SDMA-first, compute-first, generated-first, ordered composed XGMI endpoints and
forwarding/protocol adapters remain separate integration work. These lower KFD
constructors do not enforce per-allocation request witnesses.

Mutable-ledger and adapter refinement, complete bootstrap/metadata costs, native
fault/high-depth/multi-device qualification, physical overlap and matched
HIP/HSA measurements remain open. MI300X access failed at hostname resolution
before remote entry, so no remote artifacts or new native results were created.
Accepted milestone states remain unchanged.
Dual-remote delivery is recorded separately against the signed commit to avoid
self-referential evidence.
