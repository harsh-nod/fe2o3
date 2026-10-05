# Working Multi-GPU Critical Path

Priority refresh: 2026-10-04. This is an implementation work order, not an A3
completion claim. Native agents reviewed Context admission, backend custody and
existing MI300X evidence independently. Primary owns edits, integration and tests.

## Immediate Priority

The [committed host binding and application checkpoint](evidence/dev-production-host-binding-2026-10-04/README.md)
now passes the genuine installed CPU application campaign: selected device
compilation, issuer/anchor publication, Worker finalization, ordinary typed host
compilation, FD195/current-record audit and retained remote conditional proof.
The proof manager and compiler coordinator remain live through the successful
application. This closes the composed admission prerequisite, not native GPU
execution, A3/A7, full HIP/HSA parity or a general formal-verification claim.

The [optional two-GPU fixture](evidence/dev-two-gpu-application-fixture-2026-10-04/README.md)
is now implemented and CPU-qualified. It retains one artifact across both
generated fills and exact result receipts, uses charged-result staging, requires
two native peer retirements plus full payload/source/guard checks, and inspects
drain, native shutdown and credit refund before emitting its hardware success
record. The device kernel is unchanged. **This branch has not executed on GPUs.**

The [separate hardware qualification harness](evidence/dev-two-gpu-qualification-harness-2026-10-04/README.md)
is implemented. `genuine-two-gpu` requires explicit hardware UIDs, checked topology
and both directed routes, carries only the selected render nodes plus KFD through
all four private namespace layers, and validates one matching execution record
after successful child exit. Its read-only selector passes on MI300X for render
nodes 128 and 136; this does not establish device idleness or execute GPU work.

Current work order:

1. Prepare the complete pinned compiler/proof/offline-cache input bundle and make
   the required real-root service launch available on MI300X. Read-only
   checks reach the host, but the fixed services are absent, noninteractive sudo
   requires a password and direct root SSH is unavailable. Compiler/tool paths and
   four setup DSO hashes match the qualification host; that is not the complete
   deployment closure. Do not bypass admission or weaken device permissions.
2. Select two freshly observed free physical GPUs, resolve their hardware IDs and
   expose only their render nodes plus KFD in the private application namespace.
   The harness preserves application UID1000 and derives necessary numeric GPU
   groups. KFD remains process-global; this is not kernel-enforced GPU isolation.
3. Execute 65-element fills with G128/WG64 on both devices, then stage their actual
   completed bytes into PUBLIC native peer copies in both directions. Require two
   native peer completions, complete payload/source checks, separate sentinel
   destinations and intact guards. Require explicit successful drain/release;
   timeouts, process exit and quarantined work are not native settlement.
4. Exercise second-invocation and transfer-deadline failure controls. Require no
   hardware success record and inspect owned shutdown/quarantine.

Latest harness qualification: **38 unit tests passed, 12 ignored**, strict Clippy,
shell parser controls and four-layer synthetic character-node carriage passed.
Hardware mode rejects absent KFD topology before creating a cgroup, without
falling back. The live MI300X read-only selector passes 1/1, and a fresh genuine
zero-argument campaign passes 1/1 in 458.15 seconds including isolated deployment
and cleanup. The GPU execution branch, A3/A7 and performance remain unqualified.

Latest fixture qualification: **4 pure tests passed**, full host typecheck and
strict binding-only Clippy passed. A fresh genuine zero-argument campaign passes
1/1 in 484.47 seconds including isolated deployment/cleanup with the GPU branch
compiled into the binary. No GPU or performance run is claimed.

Prior frontend qualification: **436 unit tests passed, 7 ignored**, plus the
real static Cargo warm-cache test and strict scoped Clippy. The genuine campaign
then passed 1/1 in 470.89 seconds including isolated deployment/cleanup. Two existing
source-string contract assertions still fail in a broader integration check;
the new host/device boundary check passes. See the evidence for exact limits.
Those broader suites were not rerun for this fixture-only change.

General performance tuning, wider opcode/collective coverage, additional GPU
counts and same-process reopen remain behind this two-device correctness path.

## Prior Integration Checkpoints

The records below retain each earlier checkpoint's evidence and then-current
gaps. The immediate work order above supersedes their historical next-step wording.

The fresh [genuine application campaign](evidence/dev-genuine-application-startup-2026-10-04/README.md)
now starts the actual installed manager/coordinator with fresh compiler/anchor
keys, authenticates coordinator readiness, and reaches the selected kernel in
the protected rustc backend. It exposed and fixed three production blockers:
executable mode changes after ownership transfer required absent `CAP_FOWNER`,
locked Cargo queries duplicated `--frozen`, and `run` sent its host binary target
to AMDGPU instead of compiling the device library. This is progress through
startup and compilation, not successful application admission.

The compiler-time proof placement blocker is now resolved in the genuine installed
campaign. Cargo descendants retain the mandatory exec-notification filter, while
the independently admitted broker executes generated proofs under the existing
verifier profile. The [integration checkpoint](evidence/dev-compiler-proof-integration-2026-10-04/README.md)
passes real proof-backed lowering, compiler issuer acquisition/publication and
Worker finalization, then fails in ordinary host-library compilation because its
typed kernel has no binding. This is not successful application admission.

The original-compiler custody prerequisite is now implemented: the selected
wrapper captures the child immediately after spawn, transfers from its retained
pidfd, and keeps that owner through child reaping and managed commit/revocation.
Compiler completion rejects readiness without original child custody; application
readiness retains its separate outer lifecycle owner. Regressions cover wrong
children, changed descriptor flags, missing custody and post-reap validation.
The [verifier-owned compiler-proof transport](evidence/dev-compiler-proof-broker-2026-10-04/README.md)
is now implemented and component-tested. It authenticates original sealed broker,
wrapper and compiler processes, uses sealed generated source and bounded output,
and retains the existing verifier-owned proof producers and receipt join. The
one-shot spawn retains original compiler pidfd custody and drops Command aliases;
terminal client failure shuts down the endpoint and cancels/reaps the proof tree.
Private descriptors fit the protected launcher's minimum file limit. Local proof
execution and its strict seccomp contracts are unchanged.

Cargo and the compiler now use this transport. One authenticated preparation
consumes the original Cargo exec-permit owner without changing the initial four-FD
response. The original selected rustc child receives the exact sealed invocation;
proof work runs alongside the source/ISA observer. Protected compiler custody
retains one brokered runtime through both proof joins and publication. The original
canonical invocation FD199 remains owned through issuer observation; closing it
early caused the genuine campaign's first publication failure. The harness now
requests V6, matching the production handoff, without relaxing version checks.
All 420 Cargo, 524 compiler and 188 verifier tests pass, with 16 total ignores.
Production-shaped negative prefix/second-request sequencing tests remain additional
coverage work; the genuine campaign qualifies the positive composed path only.

Next, propagate the original committed device-library binding to the exact
matching host library through a retained, authenticated host projection. The host
phase currently disables wrappers, and independently deriving host Cargo metadata
does not reproduce the device binding. The genuine V6 rerun confirms the typed
host kernel rejects at this missing-binding boundary. Retain the committed envelope
lease and original invocation, not a new random build session or a lock-holding
current token across host Cargo. Match only the exact ordinary host library;
main, dependency, build-script and test units must not receive its binding.
Then qualify FD195/current-record audit and
retained conditional proof in the ordinary application before two-GPU fill and
bidirectional PUBLIC XGMI. General performance work, wider opcode coverage and
additional GPU counts remain deferred. WSL SSH still times out, but a read-only
Windows OpenSSH check now reaches MI300X. Use that route for hardware qualification
after application admission; no new hardware correctness or performance result
is claimed. This checkpoint performed no MI300X action; its isolated deployment
and all four campaign cgroups were removed.

## Implemented Foundations

The [conditional native invocation join](runtime-conditional-fill-invocation-v1.md)
is now implemented: exact charged generated packing, full64 coverage, selected
finalized entry, mandatory native storage constraint and shared remote custody
through the existing completion carrier. Production application admission and
two-GPU execution are still unqualified. Explicit
[Cargo/supervisor custodian routing](runtime-custodian-supervisor-route-v1.md) is
implemented with the run-only `--application-proof-custodian` option. The next
deployment work remains the complete production package/activation profile;
the fresh private-layout campaign above is now exercising the actual entrypoints.
The old composed-startup
fixture uses synthetic carriage and cannot qualify production FD195 verification.

The [independent proof manager](runtime-proof-manager-v1.md) now implements the
authenticated registered-owner/controller join and cooperative coordinator
bootstrap. Its fixed manager owns controller launch, ReadyOffered custody and
post-coordinator-loss retention. The public launcher no longer accepts arbitrary
raw registration tuples. Ordinary host/supervisor route selection is implemented;
composed production admission and two-GPU application qualification remain
the next gates; do not equate manager activation with proof or GPU execution.
The standard compiler-only deployment bundle still excludes the manager/controller
closure and its approval records. Package and activate those resources before
switching the ordinary host/supervisor route; the isolated manager fixture does
not establish production deployment readiness.

The next deployment change must extend the closed compiler-only bundle/installer
using a distinct proof profile, preserving the existing V1 inventory. Include
the proof manager, application controller and manager unit; install
the analyzer's immutable loader/DSO closure and the protected Verus runtime; then
provision application approval before manager approval against their final paths.
The manager approval also pins the installed coordinator image. Development-tree
analyzer measurements cannot be reused after relocation because they include
mapped paths, and the manager service hides home directories. Activate the manager
independently; service ordering alone does not start it.

The [production proof provisioner](runtime-proof-deployment-provisioning-v1.md)
now separates non-root final-resource inspection from independently pinned root
approval installation. It validates installed images and credential separation,
and publishes the application/manager approval pair atomically. The static proof
builder includes the manager, controller and provisioner. This does not complete
the analyzer/Verus closure package or establish ordinary application admission.
The [installed-resource campaign](evidence/dev-installed-proof-resources-2026-10-04/README.md)
now positively qualifies the real inspector twice under the exact proof account
with homes and source aliases hidden. It installs the identical candidate and
runs the actual fixed controller through ResourcesReady, real conditional proof,
retained probing/EOF quarantine and three rejection cases in that same layout.
The existing Worker already links LLVM/LLD statically; only its seven external
base DSOs need canonical loader placement. No analyzer rebuild or new runtime
wrapper is required for this gate. This does not qualify production package
activation, authenticated manager registration, genuine compiler receipt/currentness
admission or GPU execution. The genuine campaign above now passes compiler receipt
publication and Worker finalization, but fails in host compilation. Clear the
original device-to-host binding blocker, followed by application admission and
two-GPU fill/XGMI.

Expedite one ordinary admitted application across two selected GPUs. Native
multi-device routing, DATA ownership, issue/completion and PUBLIC XGMI already
have scoped hardware qualification. Do not add more routing wrappers or expand
opcode coverage before clearing application admission.

The [consuming application proof client](runtime-application-proof-client-v1.md)
and explicit host remote conditional artifact are now implemented. Transport
qualification includes cross-UID failure controls and the exact Cargo syscall
allowlist installed after helper entry. The composed observed manager / actual
proof / production FD195 host path is not yet qualified. The conditional invocation
join to per-device native preparation is implemented; qualify it in that composition.
The [fixed-controller execution campaign](evidence/dev-proof-controller-execution-2026-10-04/README.md)
now passes real post-exec analysis/proof, retained-owner probing, three mismatch
controls and live-Verus cancellation. Do not substitute another isolated identity
or routing checkpoint for the remaining application/native join.

The independently installed root launcher is now implemented in
`fe2o3-proof-custodian`: it opens the fixed controller, analyzer and Verus runtime,
retains original process custody and contains each proof tree in a fresh cgroup.
Its [pollable lifecycle](runtime-proof-custodian-polling-v1.md) supports cooperative
startup, proof execution and cancellation on the originating root thread. The
independent manager now consumes the broker's authenticated registered owner and
uses that lifecycle for a staged application session. The consuming host client
and remote conditional artifact now exist; qualify their composition with the
implemented per-device native invocation join.
Keep general performance work and additional GPU counts out of this critical path.

The [staged application controller](runtime-application-proof-controller-v1.md)
now implements resource Ready, exact root activation, two-FD application requests,
retained proof probing and quarantine. Its root staging primitive now requires the
manager's authenticated received registration; the staged capsule alone cannot
authenticate that observation. The generated-only
multi-device native-peer constructor is also implemented without granting generic
compute authority. Next priorities are composed host proof qualification and
two-GPU conditional application qualification.
The [application-controller campaign](evidence/dev-application-proof-controller-2026-10-04/README.md)
now passes genuine proof/probe/EOF-quarantine and three application rejection cases,
plus all 18 existing launcher regressions. Its registration records are component
fixtures, not completion of the authenticated ordinary-application gate.

The [published custodian handoff](runtime-application-custodian-handoff-v1.md)
now adds a distinct authenticated broker route and moves the original published
application owner out of issuer/registry cleanup. Identity and capacity reservations
survive extraction; the legacy route remains unchanged. The sender-side pending
owner now joins the approved manager transport and staged controller through
Ready/Activate. Explicit ordinary host/supervisor selection now joins the
consuming proof client; the legacy application route is not a fallback for remote
proof. Do not add more standalone identity or route wrappers in place of that join.

1. Completed: retain the genuine conditional fill compiler owners, exact finalizer
   lineage and authenticated analyzer/model association in the host verification
   request. The native current-publication audit passes, including substitution
   and publication-replacement controls; this is not launch authority.
2. Completed: generate the conditional semantic MIR, neutral KIR and target KIR
   relation from their actual operand graphs, execute the shared machine bodies
   under the protected Verus runtime, and retain a distinct signed refinement
   receipt. This proves the conditional projection, not native launch premises.
3. The owned pending artifact now retains the original compiler/target/analyzer
   refinement, publication token, independently rechecked finalizer and fresh
   exact compiler-current-record audit. Independently pinned production deployment
   admission now retains fixed-path configuration provenance through that audit.
   Root-side process/publication observation is implemented and locally qualified
   across UIDs. The authenticated broker channel and private issuer guard now retain
   original occurrence custody and both publication-lock descriptions through durable
   commit, with exact issuer containment before failure-path lock release. The
   production coordinator registration table and static-launcher wiring are now
   implemented, with mandatory observer admission before issuer recovery/readiness.
   The empty-capability issuer cannot perform that inspection itself. Keep signing
   unprivileged; do not relax the host's ptrace policy.
   Qualify genuine compiler receipt acquisition and verification through the deployed
   issuer and separate anchor; test-key responses do not complete that qualification.
   Preserve unconditional admission; never expose the pending state through existing
   unconditional executable/load APIs.
4. Consume exact packed/prepared full64 coverage, storage/fixups, selected device
   and publication currentness before creating private invocation authority.
   The native spatial/storage prerequisite is now implemented as a tightening
   constraint on the existing fixed packet. Preparation checks the actual patched
   272-byte kernarg and original retained code/coherent-host output allocations;
   submission requires that same prepared owner and first dispatch generation. This is not
   the consuming join with pending compiler/proof custody or launch authority.
   Device-local fill output remains excluded from this first conditional profile
   until its retained allocation identity includes the memory-session association.
5. Implement the ordinary admitted executable, then qualify fill on each GPU,
   completed output -> staging -> settled H2D -> PUBLIC XGMI -> full guarded
   readback in both directions, including N=65 with G=128. `examples/fill` still
   returns Unsupported and its read-write f32 kernel is not the proved write-only
   u32 index fill. Reuse the genuine conditional-fill body, with compiler-derived
   namespace, not the fixture's captured namespace. The native fill output must
   be exactly 260 bytes at offset zero. Put its actual completed bytes into a
   separate guarded staging frame: this checks transfer guards, not guard storage
   around the native fill allocation. A second write-only fill does not prove
   consumption of peer input. Existing copy-only and fixed qualification-kernel
   smoke tests do not complete this ordinary-application gate.

Ordinary application admission also needs a reviewed initialization design.
`cargo-fe2o3` installs the permanent no-fork application filter in `pre_exec`,
before the first application instruction, not after its descriptor ACK. There
is no existing pre-ACK proof initialization hook. Local Worker/Verus execution
cannot run inside that profile. The production auditor's missing credential,
filesystem and sealed-capability operations now have narrow syscall admission:
filesystem ID setters admit only the nonmutating query and memfd creation admits
only the existing sealed-image producer's flags. No process creation, socket
creation or additional exec is admitted. Actual installed production auditing
under this filter remains a separate qualification gate. Preserve
the sandbox and original proof custody; neither an arbitrary initializer nor
imported signed proof bytes solve this boundary. Genuine receipt/current-record
qualification outside that application profile remains intermediate CPU evidence,
not completion of the ordinary two-GPU application gate.
There is no existing parent-to-application transfer for the concrete owned proof.
The next proof change needs an independently authenticated retained-proof
custodian and a private move-only remote owner binding the exact artifact,
obligation, original process occurrence and fresh session challenge. Keep FD195
exclusive to compiler-currentness auditing. Custody must outlive both selected
devices' invocation settlement; receipt serialization alone is insufficient.
The inert canonical fill subject is now implemented after exact host association
and retained by successful pending admission. It commits original compiler/proof
roots but is neither occurrence identity nor a remote owner. Independent closed-fill
host-contract derivation is now implemented from the original lineage-bound ABI
receipt, descriptor and checked semantic/KIR inputs. Pending admission compares
that result with the marker declaration before service use. The positive fixture
now uses the actual macro-generated ABI/contract; a same-name/same-binding synthetic
profile rejects without consuming the inherited endpoint. Generic request
preparation and the existing exhaustive error enums remain unchanged.
Complete immutable compiler-closure checking is now factored out of ordinary
admission. `check_worker_v3_compiler_closure_v1` canonical-decodes the V2 envelope,
replays the finalizer and selects the exact descriptor from borrowed envelope and
HSACO bytes, without filesystem access or publication-lock acquisition. The checked
closure can independently derive the fill subject from an original executed proof
without a marker or caller-supplied contract. Application requests now expose their
original V2 evidence borrow while the same current token is held. Copied evidence
does not acquire the original runtime evidence view's provenance or currentness.
The fixed custodian can reuse these checks, but still needs authenticated original
application occurrence, a fresh session and retained remote proof ownership. Do not
reacquire the application's held publication lock in that service.
The [marker-independent retained producer](evidence/dev-retained-fill-producer-2026-10-04/README.md)
is now implemented as
`execute_retained_worker_v3_conditional_fill_v1`. It owns the exact envelope and
HSACO, reconstructs the checked compiler closure, executes authenticated machine
analysis and the protected conditional proof, and retains the original proof and
inert subject. It rejects oversized inputs before replay/copy and shares one
absolute deadline across stages. It does not authenticate deployment, application
occurrence or compiler origin, and it cannot become an unconditional executable.
The next service work must retain this concrete owner, not reimport its receipt.
Root application observation currently retires on application exit/endpoint EOF;
leased proof custody must instead survive until exact invocation settlement or
remain quarantined. Do not place that ownership in the compiler issuer's table.
For the next transport increment, reuse registration instead of adding a second
application handoff handshake: stage a duplicate of the exact peer in an
independently measured sibling custodian, keep it inactive, and extend the
root-authenticated Ready message with its deployment/session identity and pidfd.
The filtered compiler coordinator cannot launch the unfiltered proof role directly.
Provide an independently approved fixed unfiltered launch boundary and an actual
resource opener/launcher; an identity-only deployment manifest is insufficient.
Only root sends Ready. After successful Ready delivery, activate the custodian
and retire root's peer alias; proof packets then require that custodian's exact
per-message credentials and fresh nonce. The staged controller protocol is now
implemented, as is the authenticated coordinator/manager handoff. The ordinary
application's consuming proof client is implemented; production route selection
and composed qualification remain open.
The first small guarded-buffer pipeline can reuse direct context polling or the
existing current-thread engine. This does not qualify larger or threaded variants:
shared-memory initialization/verification at 64 MiB or above can spawn scoped
workers, and the threaded async engines also conflict with the current profile.
Keep proof execution outside the 30-second registration/ACK path. The inherited
compiler auditor's separate 30-second absolute deadline begins at admission, not
its first request; admit it immediately before the audit, before the long proof. Bound
custodian startup, analyzer execution, proof and audit together within the existing
300-second issuer/observer sessions. A 180-second proof bound alone is insufficient.
The current-record audit reads the protected issuer ledger and anchor; it does not
reacquire the application's held artifact-publication lock.

Protected compiler-origin/currentness and native entry/storage/completion
associations remain required. Test-signed carriage and an analyzer/model match
cannot replace them. Defer general optimizer proofs, additional opcodes, more
GPU counts, zero-copy generated-to-peer transfer and performance campaigns.

The [owned-refinement checkpoint](evidence/dev-owned-fill-refinement-2026-10-03/README.md)
removes borrowed-proof lifetime constraints without self-reference or reimport.
Retain one compiler-service admission for the artifact: its inherited current-record
connection is one-use. Discharge each selected GPU's invocation separately without
treating copied signed records as original evidence custody.
The [pending-admission checkpoint](evidence/dev-pending-fill-admission-2026-10-04/README.md)
adds the concrete one-use service transaction under retained publication currentness.
It remains distinct from production deployment approval and invocation authority.
The [inert-subject checkpoint](evidence/dev-conditional-fill-subject-2026-10-04/README.md)
adds fixed canonical matching data without replacing any original owner or changing
validation order. Its fresh six-case protected campaign remains test-key-backed;
authenticated cross-process custody is still required before this identity can
participate in service admission.
The [closed-contract checkpoint](evidence/dev-conditional-fill-contract-2026-10-04/README.md)
adds independent reconstruction of the exact explicit host ABI and launch contract.
The ABI receipt's domain-separated identity must match the original target lineage;
a raw descriptor content hash is not that receipt identity. A copied contract digest
is canonical equivalence data, not original ABI provenance or launch authority.
The [compiler-closure checkpoint](evidence/dev-compiler-closure-check-2026-10-04/README.md)
shares immutable validation with ordinary admission and independently reconstructs
the same fill subject. It is not a custodian deployment or cross-process owner.
The implemented registration path authenticates the original application before its
descriptor ACK, on a dedicated proof endpoint rather than FD195. Cargo reads that
ACK pipe until EOF: root-side inspection must close any duplicated ACK writer
before allowing acknowledgment, while retaining the original pidfd and other
occurrence evidence. Keeping the ACK writer alive for proof custody would deadlock
startup. App-lifetime proof custody must not occupy the compiler issuer's bounded
worker slots, and process exit alone cannot prove GPU settlement.
The [application-observation checkpoint](evidence/dev-application-observation-2026-10-04/README.md)
now provides that local process/input prerequisite. It independently checks original
pidfds, parent relationship and credentials, a kernel-sealed static image and the
three current handoff descriptor slots. It retains exact envelope bytes without
decoding or recovering publication, and closes both temporary ACK writer duplicates
before returning. Post-ACK validation checks live process, executable, envelope and
directory continuity without touching the closed/reused ACK slot. An isolated
root/UID1000 campaign now qualifies ACK EOF with the observation alive, source
substitution, changed flags/bytes, same-byte executable replacement and application
death while its parent remains live. This is neither authenticated registration nor
the no-fork application/proof-custodian deployment.
The dedicated inherited proof endpoint is now bound as the fourth input on Cargo,
host and root-observation sides. Registration authenticates messages against the
original application pidfd and retains a fresh bounded application session
independently of issuer lifetime. The [startup custody checkpoint](evidence/dev-application-startup-2026-10-04/README.md)
adds Cargo's separate post-spawn transition before compiler readiness:
it drops parent ACK and test-readiness writers, captures the original unreaped child's
pidfd, completes sandbox admission once, and retains the complete occurrence,
descriptor coordinates, expectation and challenge through ACK and delayed cleanup.
The binding is allocated before spawn and moved without cloning. Fast ACK-and-exit
is valid; this custody is not a live-process observation or registration authority.
Application compiler-service transfer now duplicates the captured original child
pidfd. A private-field move-only owner also captures the original Cargo pidfd;
it has no production raw-descriptor constructor or extraction. Live transfer
revalidates current-parent waitability against the original pidfd, while cleanup
may retain descriptor/history custody after exit or reaping. The ordinary compiler
PID-based path and its two-right supervisor wire remain unchanged. The active
four-right application registration uses duplicates of these original handles.
The [registration-prerequisite checkpoint](evidence/dev-application-registration-binding-2026-10-04/README.md)
qualifies this transfer and the inert four-input binding. Both original handles
and protocol occupy the same pre-spawn allocation through cleanup, without a new
post-spawn allocation. That checkpoint did not install the dedicated proof endpoint.
The [proof-endpoint checkpoint](evidence/dev-application-proof-endpoint-2026-10-04/README.md)
now creates the Cargo-owned nonblocking PASSCRED pair before spawn, closes Cargo's
application-side alias immediately afterward, and retains the counterpart until
single-use transfer or failure cleanup. Host admission claims and checks the exact
fourth occurrence,
including strict environment/profile agreement, without widening the sandbox.
Root observation derives all coordinates and process assertions from the complete
registration binding and checks Cargo creator credentials, reversed abstract
addresses, distinct socket objects, original pidfds and source-slot continuity.
It retains only application-endpoint facts, never an application-side socket alias.
The historical three-input observation qualifier remains separate; current
production host consumers require all four inputs. Static single/roster
admission and cross-UID ACK EOF/proof HUP are qualified locally; this is not installed
authenticated application registration, proof custody, or GPU launch authority.
The join must authenticate Cargo's
spawn/sandbox supervision and protected service isolation, neither of which follows
from the process snapshot alone. Then deploy the fixed keyless custodian, join its
original proof to native invocation premises, and run the selected two-GPU pipeline.

The registration design reuses the supervisor/root registry without another
listener. The root registry, observation handshake and production
Cargo/supervisor/host activation are now implemented as described below.
The [composed startup campaign](evidence/dev-composed-application-startup-2026-10-04/README.md)
now passes descriptor, roster, delayed, clone3-fallback and preparation-cancellation
cases under the actual application filter. It also fixes the empty-cap supervisor's
inaccessible namespace inspection of its non-dumpable child: fixed pre-exec child
code checks its own namespaces before readiness, preserving every other launch gate.
This test-key/component fixture is not measured deployment or a genuine FD195 audit.
Remote proof custody and its consuming native join are now the immediate implementation
priority; deployed compiler acquisition remains a separate required qualification.

1. Completed prerequisite: the inert canonical application binding is exactly 840 bytes
   containing the full compiler supervisor handoff, exact four-slot occurrence,
   descriptor coordinates, envelope expectation and challenge. Bounded decoding
   validates framing, all nested records and expectation/occurrence association;
   coordinates must be distinct and exclude FD195. Cargo now creates a nonblocking
   CLOEXEC seqpacket pair before spawn, enables PASSCRED, eagerly binds both endpoints
   and includes slot 4 before encoding the occurrence. Host claim/cleanup and root
   observation now validate the same four-input profile. FD195 is unchanged.
   Endpoint possession and local observation are not authenticated registration.
2. Implemented [transport/admission prerequisite](evidence/dev-application-supervisor-handoff-2026-10-04/README.md):
   a distinct application kind with exactly four rights:
   compiler peer, duplicate of captured original app pidfd, proof peer, and original
   Cargo pidfd. Ordinary compiler handoff still requires exactly two. Authenticate
   Cargo's control credentials and the complete binding; do not numerically reopen
   the app PID or downgrade an application to the ordinary profile.
   The new client launch/pending and supervisor accepted types are move-only and
   have no conversion to the compiler-only types. Client transfer independently
   reconstructs the embedded compiler handoff and checks slot 4 against the original
   prepared pair. Supervisor admission validates both process tokens and direct
   parentage, retaining a candidate Cargo-created proof peer. It cannot independently
   establish the remote slot-4 counterpart: that still requires root observation.
   Cargo and host now activate this profile; their public APIs pass the composed
   component campaign above. Measured deployment remains unqualified.
   Supervisor session dispatch classifies the
   ordinary/application profiles atomically as described below.
   The accepted handoff alone is not
   authenticated root registration, application readiness, or GPU launch authority.
3. Implemented [root application registry](evidence/dev-root-application-registry-2026-10-04/README.md):
   root `RegisterApplication` uses the existing two-right compiler registration
   transport and marks its preparation AppExpected. `AttachApplication` references
   that exact unexpired preparation and transfers only proof peer and Cargo pidfd.
   Duplicate the original app token from the prepared observer's retained client,
   with pre/post validation. This keeps the root transport's two-right limit intact.
   Install a separately bounded app entry, including separate containment custody,
   before replying Installed. Missing/duplicate attachment and attach-after-bind
   reject. Compiler eviction or issuer exit cannot retire the app entry.
4. Implemented root gate and shared bounded handshake codec: Installed returns a
   root-created observation-readiness reader and a distinct publication writer.
   Root retains the observation writer and waits for
   authenticated app Hello, exact observation, matching Accept, and issuer binding.
   It writes/closes the independent observation gate, then waits for the supervisor's
   exact session-bound publication record and EOF before sending app Ready. The
   supervisor commits this reverse gate only after sending Cargo's dedicated
   readiness record. No fallible issuer-liveness check follows that commit: an app
   may then ACK, complete its one-use audit, or exit immediately. The publication
   writer never enters the fourteen-input issuer ABI. The separate application
   table retains no compiler peer and permits only one initial observation per
   registry iteration. Registered observation-only EOF retires without signaling
   a still-live app; startup failure contains the original app before release.
   The [supervisor startup checkpoint](evidence/dev-application-supervisor-startup-2026-10-04/README.md)
   now seals compiler registration, full application binding and gate into one
   opaque owner. Atomic session dispatch accepts only 184 bytes/two rights or
   840 bytes/four rights, with no probe/fallback receive. The private application
   route preserves mandatory registration and issuer binding, the unchanged
   fourteen-input issuer ABI, and one startup deadline through launch/readiness.
   The public publisher awaits the gate outside the registry mutex after issuer
   readiness, rechecks issuer and original root continuity, and sends only the
   dedicated 208-byte application record. Gate failure retains exact issuer
   cancellation/reaping custody. The client requires the full registration join,
   terminal EOF and the original transfer deadline; ordinary readiness cannot
   complete it. Local fixtures qualify the public publication/cleanup transition,
   not a deployed root/supervisor/issuer/application startup.
   The [application bootstrap checkpoint](evidence/dev-application-bootstrap-2026-10-04/README.md)
   implements the consuming client Hello/Challenge/Accept/Ready API and a shared
   poll-only received-root pidfd owner. It authenticates actual per-message root
   credentials, the original pidfd target, complete local inputs, exact app/Cargo
   identities, endpoint coordinates/object and both fresh nonces. All phases use
   one absolute deadline; failed admission closes the original endpoint. Broker
   and application now share the strict target/procfs parsers, with broker waitid
   and signal-interruption semantics preserved. No application syscall is added.
   The [startup activation checkpoint](evidence/dev-application-startup-activation-2026-10-04/README.md)
   qualifies the Cargo/host changes and reverse publication gate in component and
   static-filter campaigns, not their complete production startup composition.
   Cargo now transfers the actual four-right application profile and accepts only
   its dedicated readiness. Its proof state distinguishes prepared, retained,
   transferred and absent compatibility custody; transfer is single-use and leaves
   the original child/reaper cleanup intact. One absolute startup deadline covers
   child admission, transfer, supervisor response and strict ACK/EOF.
   Both production host consumers now require four inputs and root registration
   after local admission, before ACK, even with test features enabled. One original
   publication token spans registration and the post-Ready descriptor/currentness
   checks immediately before ACK. Cargo does not reacquire that lock until app exit.
   The returned descriptor/roster retains the registered endpoint and original root
   pidfd through native unload. A live root that closes just this session also
   fails continuity. Envelope-only tests use explicit feature-gated fixture APIs,
   never a production fallback.
   Authenticate socket creator, reverse addresses and distinct per-message sender
   credentials against original pidfds. Commit observation before either readiness
   response. The new readiness pipe is not Cargo's ACK pipe: temporary ACK writer
   duplicates must close before return. Current pre-ACK admission does not use FD195;
   allowing issuer service before this gate avoids a future dependency cycle.
5. Qualify wrong slots/addresses/credentials, reordered and stale pidfds, wrong
   registration/profile, replay, capacity, backpressure, timeout and shutdown.
   One stalled app must not block the registry, and ACK EOF must remain observable.
   The positive production-context component startup now passes under Cargo's actual
   pre-exec filter, including delayed registration and immediate post-Ready exit.
   Fixed keyless manager/controller custody is now joined. Next connect the
   consuming application proof client and conditional native preparation.

The bootstrap uses no sandbox-forbidden waitid, pidfd reopening or socket shutdown.
Its root/UID1000 campaign includes live separate-root success, queued Challenge
and Ready from an already-reaped root with a retained peer alias, foreign Ready
senders, wrong pidfds/inputs, exact endpoint substitutions, malformed packets and
timeout, plus retirement of a registered endpoint by a still-live root. Helpers
install a focused process/socket syscall denylist after libtest
startup; this is not qualification of Cargo's full inherited pre-exec allowlist.
The earlier strict static negatives and explicit envelope-only positives do not
replace the now-qualified composed startup campaign or remaining deployed audit.
No new publication recovery or lock acquisition belongs in
registration. Root process identity alone does not measure the fixed custodian.
Application exit, EOF and containment remain distinct from GPU settlement.
The application must send the first authenticated Hello after claiming slot 4:
otherwise root inspection can race its required CLOEXEC transition. Root checks
that message against the original app token before observing inputs. The later
challenge/accept/ready transcript must bind both fresh session nonces and the exact
registration; app-side root pidfd inspection must use the permitted poll-only path.

The [reviewed-macro checkpoint](evidence/dev-macro-admission-2026-10-04/README.md)
closes the inherited workspace-local macro source-pin drift after independent delta
review and qualification. New exact structural tests cover the complete owned and
borrowed adapters, accounting, argument ordinals, mapped identities and unsafe trait
assertions. Nine AST mutation controls and mixed unsupported profiles pass alongside
macro/downstream and host runtime tests under frozen hashes. The external-source
pin remains independent and unchanged. This admits the reviewed macro tree, not
new host/runtime authority or an ordinary production application deployment.
The [native-fill checkpoint](evidence/dev-conditional-native-fill-2026-10-04/README.md)
checks exact machine shape, patched arguments and original native owner/session
association. Its one-generation constraint is not the pending-proof admission join.
The [production-deployment checkpoint](evidence/dev-production-deployment-admission-2026-10-04/README.md)
adds independently pinned configuration, namespace/path continuity and a concrete
production auditor factory. Its 21 isolated cases use actual root-owned fixed paths
but test-key responses, not a genuine deployed compiler/issuer campaign.
A separate owned-process permission witness confirms that the current distinct-UID,
empty-cap issuer profile lacks remote inspection permission. A narrowly scoped,
authenticated observer handoff is required for production acquisition and is now wired.
That handoff must cover the complete occurrence operation, not just process
snapshots or an artifact-directory descriptor: the issuer also lacks access to
the user's protected publication directories and lock files. Reuse the existing
Production-slot reconstruction and currentness checks on the root side, retaining
the original observation, publication lease and current token. The issuer needs
an authenticated private-channel lease bound to its exact session and fresh
challenge. Keep the root-held token through signing and ledger commit; failure
handling must contain the exact issuer before releasing that token. This is the
production integration boundary; qualification scopes are separated below.
The [root-observation checkpoint](evidence/dev-root-occurrence-observation-2026-10-04/README.md)
now implements that local occurrence owner with one unbroken nonblocking publication
lock and no client-state repair or lock creation. Two isolated cases qualify root
observation of a UID1000 private publication and real cross-UID lock contention.
Their waiting-process/synthetic-handoff fixture is not genuine compiler acquisition.
The authenticated coordinator session table is now integrated; the root observation
owner alone still cannot sign.

The [lock-retention checkpoint](evidence/dev-observer-lock-retention-2026-10-04/README.md)
adds that transfer prerequisite, not the channel:
`CompilerModuleHandoffLockRetentionV3` duplicates the original named-lock OFD and
directory-flock OFD from a nonrepairing descriptor-root observation. It never
reopens paths, copies the semantic token, or imports publication authority.
Ordinary pathname-backed observations are deliberately rejected, since their
third path guard cannot be omitted. Both exporter and importer use coordinated,
close-only destruction. The importer checks descriptor shape only; authentication
and exact operation binding remain mandatory in the future private transport.
Retaining this complete lock set in the issuer guard is necessary because killing
or checking the root coordinator cannot make revalidation-to-commit atomic.
Root crash closes its token without running its containment destructor.

The [authenticated channel checkpoint](evidence/dev-observer-channel-2026-10-04/README.md)
now implements the broker-side root owner, private remote guard and durable issuer
integration. Every packet binds exact kernel sender credentials, original live
process identities, immutable launch/session identities and a fresh operation
nonce. Begin carries both original lock descriptions. Errors poison the entire
attached issuer admission; successful Finish follows commit and its continuity
check. Root rechecks the occurrence before sending delayed responses and contains
the exact issuer before releasing active custody. Idle channel closure instead
allows a bounded clean exit, with no further requests admitted.

Linux credential-passing sockets autobind on send. The factory eagerly kernel-binds
both connected endpoints before exposure and pins both exact abstract addresses;
it creates no filesystem socket and does not relax ordinary unnamed compiler-peer
admission. These are CPU/subprocess tests with a waiting-rustc-shaped fixture,
real observed publication, and test-key durable records, not genuine deployed
compiler acquisition, a new formal proof or GPU launch authority.

The [registration/launch checkpoint](evidence/dev-observer-registry-2026-10-04/README.md)
reuses the existing root coordinator. Supervisor bootstrap FD11 stays readiness/EOF-only;
FD13 carries private authenticated registration and FD14 the original root pidfd.
Register transfers only the accepted compiler peer/pidfd, never Cargo control.
Bind transfers the actual launched issuer's original pidfd before readiness.
A bounded 16-entry table services each observer once per iteration, retaining one
backpressured control response and continuing unrelated sessions during containment.
Unbound entries expire; bound entries retain custody until exact issuer exit.
Issuer FD12 carries the observer endpoint and FD13 the original root pidfd, with
private descriptor floor 14. Production admission requires both before recovery
and readiness and has no local-observation fallback. No observation runs at readiness,
before the compiler has published. Registry mutex acquisition and Bind share the
launch's absolute deadline. Root shutdown contains all issuers before releasing
supervisor/anchor custody. DAC/ptrace capabilities are coordinator-only.

CPU subprocess qualification covers concurrent sessions, capacity, expiry, substituted
pidfds, duplicate binding and registration loss during active publication custody.
The isolated root case uses a UID1000 waiting-rustc-shaped publication and empty-cap
UID61000 supervisor/issuer with production registry/observer/profile checks and test
keys. It is not the measured static deployment or genuine selected-rustc receipt
campaign. Cross-UID fixtures publish as the client before recording payload metadata;
changing ownership after publication invalidates the ready record's ctime binding.
That campaign remains next, followed by invocation admission and two-GPU
fill/copy/readback. No new GPU run, formal theorem or HIP/HSA parity claim is made.

The [launcher compatibility checkpoint](evidence/dev-clone-compatibility-2026-10-04/README.md)
addresses the inspected systemd 255 filter's `clone3 -> ENOSYS` behavior without
changing `RestrictNamespaces=yes`. Both launch paths now use an ENOSYS-only
x86-64 `clone(CLONE_PIDFD | SIGCHLD)` fallback, still returning the original pidfd
atomically. The calling thread blocks the complete kernel signal mask before
either clone; the child resets dispositions before unblocking. The parent takes
cleanup custody before restoring its exact mask. Restoration failure contains
the exact child before return or fail-stop, and unknown wait errors retain custody.
Local qualification applies the installed systemd helper's actual namespace filter
to the supervisor suite and private distinct-UID root launch cases, including
negative namespace probes. It is not full systemd unit startup or the measured
static deployment campaign. Those remain required before genuine compiler receipt
acquisition, followed by exact invocation admission and two-GPU application testing.

Measured deployment startup has now been attempted locally. It found and fixed
two real integration defects: the SquashFS no-xattrs bit and the missing read-only
superblock context before opening the read-only loop device. The WSL kernel has
SquashFS but not its zstd decompressor, so the pinned production base still cannot
mount there. The read-only mount mechanism is separately testable with a gzip
fixture, which production admission explicitly rejects. Do not change the pinned
base or treat that mechanism test as deployed compiler evidence. MI300X advertises
SquashFS zstd support but the current SSH account has no noninteractive root path.
No shared GPU resources were allocated for these startup checks.

## Reuse What Works

The selected-pair copy controller already qualifies production deny-all kernel
constructors with native and staged copies in both directions. Finite qualified
compute/peer/readback pipelines have run on 2/3/5/7 GPUs. These paths do not need
another compiler proof to run. They also do not authorize arbitrary kernels.
See [the scoped hardware evidence](runtime-multi-device-qualification-v1.md).

Do not use the generic hardware-smoke lane on the shared MI300X host: its KFD
examples select all devices. Select explicit freshly observed free GPU identities,
bound each owned process, preserve outputs, and clean only owned resources.
No resets, disruptive fault injection or exclusive-performance claims.

## Completed Native Capability

The existing public API now supports this pipeline:

```text
A --ordered segment lists--> B --ordered segment list--> C --whole-frame D2H
```

The B-to-C list reads B's exact latest retained destination frame without a host
join, including initialized gaps outside the earlier list envelopes. The
[eight-case native checkpoint](evidence/dev-frame-segments-forward-2026-10-03/README.md)
passes prequeued and late admission, overlap and packet-tail shapes, and reversed
routes on MI300X GPUs 1/6/7. The complete runtime suite passes 2352 tests with
32 hardware ignores. This closes this functional work item, not A3.

| Lane | Scope | Acceptance |
| --- | --- | --- |
| Context and journal | Separate default-false capability; distinct frame-source origin; existing immutable frame receipt; generic source-dependency accessor without changing compute-producer meaning | Exact latest event, owner, allocation, envelope and dependency rank; source lease covers the bounding envelope; reject missing/stale events and pending destination writers |
| Native backend | Generalize existing frame-source transfer identity from scalar window to window or immutable segment plan; reuse existing list queue, packet plan and progress | Authenticate exact plan and paired ancestry; wait for successful parent completion and both original owner restorations before successor publication; preserve successor destination-frame receipt |
| Qualification and review | Extend existing Context/backend forwarding fixtures and three-GPU destination-segments witness | Final-readback-only progress; full output and guard bytes; exact callbacks/native counts; released event custody, cancellation/failure and explicit cleanup |

The first profile deliberately excludes a pending destination writer and pending
compute consumption of the new frame-derived list. Its Frame source origin does
not inherit compute-origin ordering permission. It retains whole original owners,
not per-descriptor owners. Parent quiescence or failure is not successful input
readiness. Unknown effects remain retained and cannot authorize retry.

Fifteen new CPU tests cover immutable descriptors, source-envelope gaps,
plan/owner/rank drift, released events, bounded linear ancestry validation,
failed/cancelled/Unknown parents and queued-reader rollback. The full suite also
retains unwind quarantine and independent-pair controls. Native cases use two
changed-content rounds and whole-C readback. Existing descriptor arithmetic proof
bodies are unchanged; they do not prove the new Context/backend adapter.

Relevant implementation boundaries are `context/peer_segments/custody.rs`,
`context/versions/producer_readers.rs`, `context/versions/submissions.rs`,
`kfd_backend/peer_frame.rs`, `kfd_backend.rs` and
`kfd_backend/compute_xgmi/segments.rs`. Existing fixtures are
`context/tests/compute_peer_tests/segments/forward_segments.rs` and
`kfd_backend/compute_peer/frame_segment_tests.rs`.

## Next: Application Admission

Advance the first ordinary application kernel through the existing production
constructor, then qualify compute -> native peer -> compute/readback across
selected GPUs. Genuine Rust extraction,
owner-bound captured KIR and shared argument-basis/fold proofs now exist for a
bounded checked-u32 prefix. The actual borrowed source-statement normalizer now
also has a shared-executable exact acceptance and denotation proof, with checked
schema/getter correspondence and explicit irrelevant-payload erasure. See the
[normalization checkpoint](evidence/dev-source-normalization-2026-10-03/README.md).
These proofs do not cover a complete kernel.

The [source-assembly checkpoint](evidence/dev-source-assembly-2026-10-03/README.md)
now composes the actual retained span scan/walk with source normalization and
pre-add AST/fold denotation. Its bounded private ordinal vector replaces the
source tree map, making the scan/walk linear. Terminal AST evaluation is separate.

The [KIR assembly checkpoint](evidence/dev-kir-assembly-2026-10-03/README.md)
now proves actual borrowed Constant/CheckedAdd assembly and direct evaluation.
Sparse V8 IDs map directly to origins, removing redundant dense constant scratch
and folding. Exact acceptance preserves original rows, typed outputs and the
immediately preceding literal identity. Terminal source AST validation and ABI
discovery remain open.

The checked-u32 helper lane still needs ABI discovery and terminal source
validation. Complete application admission needs semantic-to-machine entry,
continuation, memory and completion evidence, protected invocation custody and
the concrete deployment-approved Worker V3 verifier/refinement providers. Do not
replace these with qualification metadata, caller digests or an always-allow
backend. Affirmative Worker provider implementations remain test-only. Bound the
first complete profile to the actual `fill_write_only` kernel described below,
not a separate checked-add prefix. Its conditional compiler proof now survives
the singleton handoff and import. Actual packed launch coverage is now checked
against the retained compiler ABI; physical entry, output stores, termination
and exact source-to-machine correspondence still precede application admission.

The [conditional transport checkpoint](evidence/dev-conditional-fill-transport-2026-10-03/README.md)
retains the signed coverage condition through genuine Rust extraction, V9
handoff, conditional import and target replay. The old unconditional importer
and protected Worker constructor remain unchanged. The
[packed coverage checkpoint](evidence/dev-packed-conditional-coverage-2026-10-03/README.md)
now joins the exact semantic output, KIR parameter, canonical descriptor, typed
packing plan, actual length/backing/fixup and AQL grid. It retains immutable
borrows of the proof and packed owner; it creates no prepared dispatch or device
authority. The [whole fill program checkpoint](evidence/dev-conditional-fill-program-2026-10-03/README.md)
now checks the genuine source body, complete neutral KIR and independently
replayed target KIR against the same guarded index-fill behavior. It rejects
safety-only predicates, changed values/addresses, extra effects, incomplete
control flow and stale witness borrows. This is a bounded checked relation with
CPU/compiler tests, not a new formal proof or a machine execution join.
The [dispatch composition checkpoint](evidence/dev-fill-dispatch-2026-10-03/README.md)
now proves the shared full64 geometry validator, descriptor-shaped entry
construction, actual per-wave execution, unique cross-group coverage, exact
output bytes and untouched complement. Byte queries execute at most one wave;
neither storage nor validation scales with the grid. Its 44 cumulative proof
obligations include the original 30 wave obligations. Actual native entry,
patched allocation backing, scheduling, visibility and completion remain
premises, not established facts. The
[authenticated fill checkpoint](evidence/dev-authenticated-fill-2026-10-03/README.md)
now retains the actual analyzer execution with the exact inspected fill model.
An unchanged genuine compiler handoff passes the actual Worker bootstrap/replay,
finalizer and authenticated analysis. Its production 272-byte kernarg layout is
accepted with full-storage bounds, while the model reads exactly 16 bytes.
The [host association checkpoint](evidence/dev-host-fill-association-2026-10-03/README.md)
now joins those owners to the exact current-publication request, canonical compiler
descriptor, semantic kernel identity and finalized payload. The actual native
Worker-to-host audit passes; synthetic carriage is not compiler-origin authority.
It also fixes V5 correspondence wrapper erasure in both compiler import paths and
checks the retained function roster against the bound KIR. The
[conditional refinement checkpoint](evidence/dev-fill-refinement-2026-10-03/README.md)
now generates independent semantic/neutral/target expressions from actual operand
edges and proves their relation to the shared wave execution and byte projection.
Its distinct boundary retains actual protected execution and strict signed import.
The protected qualification includes six logical operand mutations and an
equivalent alias rewrite, not merely fixed machine theorems with attached hashes.
Owned pending custody and the concrete signed service transaction now pass native
qualification. The independently pinned production factory is implemented; qualify
it with the genuine deployed issuer and complete the consuming invocation transition
next. Only then
qualify admitted fill, tracked upload, native XGMI and readback on two GPUs in
both directions.

The pending owner now consumes and retains the original compiler inputs, target
lineage and authenticated analysis without reconstructing authority from signed
bytes or holding self-referential borrowed views. Actual DATA pointers are patched in the KFD
queue-dispatch preparation layer, after host preparation. Discharge alignment,
bounds, non-overflow and DATA/kernarg disjointness there before issue; a hash of the
unpatched template is insufficient. Current-record audit evidence is also not
protected compiler-key custody. The production factory now admits the fixed client,
supervisor and anchor configuration, verifies their exact policy/UID/key links, and
retains/rechecks installed-path provenance around the exchange and evidence binding.
Later invocation authorization must revalidate those original owners as well.
The existing hardened issuer implements checks for actual compiler occurrence,
signing-key custody, ledger currentness and a live independent anchor before signing.
The root-side observer now supplies the separate privileged inspection while the
issuer remains unprivileged. Genuine acquisition through that deployed composition
still needs qualification. Reuse the existing signing mechanism; do not trust the carriage's
self-selected policy or mistake FD195 socket
credentials for the transferred issuer's identity. No new signing protocol is needed
for this trusted-local-root/kernel scope.

The current Worker artifact verifier accepts only unconditional compiler proof
inputs and runs before invocation arguments exist. Do not retag the conditional
owner as unconditional. A conditional artifact profile must retain the universal
machine contract; a later invocation transition must consume coverage tied to
the exact prepared dispatch, selected device and patched memory. Reuse the
existing complete per-wave model and its now-proved dispatch projection. Join
that projection to the actual prepared dispatch, selected device and patched
storage instead of adding more isolated opcode proofs. The packed coverage
checker permits G=65, but this full64 profile deliberately requires G=128 for
N=65; rounding belongs to preparation, not to the proof checker.

## Required Multi-Device Bridge

Preparation, DATA adoption, generated issue and completion now share their
Context bodies across single-device and multi-device backends. Production
application admission and an authenticated output-to-peer composition still
need end-to-end qualification.

1. Completed: route nonexecuting preparation through the exact retained
   multi-backend child, preserving Context generation, logical device, backend
   UID, native admission, selected-child exclusion and pre/post callback
   currentness. Generic async preparation tickets and cleanup are reused.
   The [two-GPU preparation checkpoint](evidence/dev-multi-preparation-2026-10-03/README.md)
   passes both device orders on MI300X GPUs 6/7, direct reverse-order validation,
   callback errors, plain-ticket reservation rejection and owner-thread disposal.
   It creates no VM, queue, allocation or execution authority. CPU acceptance is
   2359 runtime tests (32 hardware ignores), 170 example tests and 71 doctests.
2. Nonpublishing reservation -> shell registration -> DATA adoption -> retirement
   is implemented with paired immutable global/child-local plans, private
   generated allocation routes and the existing move-only commit owner.
   Readiness and quarantine capture the exact child; routing capacity is reserved
   before transfer. Pristine adopted DATA blocks peer owner extraction and
   coherent capture even before submission indexes exist. The
   [two-GPU DATA checkpoint](evidence/dev-multi-adoption-2026-10-03/README.md)
   passes native adoption, independent retirement and primary-lane rebound in
   both device orders. This is finite native mechanics, not protected Worker
   application admission.
3. Generated issue/completion routing and owner-thread activation are implemented.
   A private global submission map retains the original child, shell and local
   receipt, including malformed returned identities and unwind. Generic
   poll/wait/drain cannot publish Ready work or report physical completion as
   delivered output. Retired DATA can release its exact receipt before shell
   disposal. The [two-GPU issue checkpoint](evidence/dev-multi-issue-2026-10-03/README.md)
   passes three native dispatches, full four-buffer readback, independent
   retirement and primary-lane rebound per device order on MI300X GPUs 6/7.
   Context/async qualification remains CPU bookkeeping and rejection evidence,
   not a protected Worker positive-path launch or a new adapter proof.
4. Completed-value staging now preserves the original charged result and uses a
   complete ordinary HostVisible write, settled upload and PUBLIC peer. The
   [staging checkpoint](evidence/dev-result-staging-2026-10-03/README.md) passes
   synthetic charged host data through real uploads and native XGMI in both
   directions on GPUs 6/7. CPU tests cover the shared receipt-matching body,
   encoding and failure custody; this is not a protected Worker positive launch
   or authenticated native completion-to-peer witness. Generated allocation slots
   remain distinct from PUBLIC allocations.
5. Qualify the complete admitted compute -> native peer -> compute/readback path
   on freshly observed free devices. Existing finite native witnesses do not
   substitute for this application-admission gate.

### Completed: Host Preparation Entry Points

The normal authenticated helpers in `fe2o3-host/generated_runtime_invocation.rs`
now include `prepare_generated_multi_context_invocation`, its async counterpart,
and `GeneratedWorkerV3ContextInvocationV1::validate_multi_context`. Existing
single-device signatures remain unchanged. Both variants reuse the original
`require_runtime_evidence`, `prepare_context_payload` and `project_persistent`
bodies, result account and decoder. No new backend abstraction or authority is
introduced. The [host checkpoint](evidence/dev-host-multi-2026-10-03/README.md)
qualifies the public APIs with genuine generated argument types and ownership,
wrong-kernel and privacy compile controls.

Missing protected evidence still rejects before Context access, argument
callbacks, budget cloning or queue admission. The source-wiring checks isolate
each wrapper independently; they are not concrete-KFD execution or a new adapter
proof. Sync preparation remains inert. The async example compiles the existing
reservation/adoption/issue/completion lifecycle while retaining failure tickets.
No new native run or production positive verifier/refinement provider is claimed.

For owner-driven staging, authenticate/encode on the caller and enqueue only
scratch plus ordinary staging handles, retaining the original output if enqueue
rejects. Require successful H2D completion before ordinary peer admission; pending
H2D writers are not peer producers. A dropped observer or timeout is not physical
quiescence. Scratch is caller-owned outside the result-credit budget.

Staging creates a new ordinary host-write version, not the original generated
allocation or pending generated producer event. Generated DATA is coherent
HostVisible storage with no native SDMA-promotion bridge. Zero-copy ownership
transfer needs separate typed transition, accounting and failure evidence.

### Next: One Real Output Kernel

The next acceptance target is one genuinely extracted, lowered and compiled Rust
device-entry kernel, then admitted compute -> native peer -> compute/readback.
Do not add more preparation wrappers or substitute a qualification-only kernel.

The [entry-layout checkpoint](evidence/dev-entry-layout-2026-10-03/README.md)
captures genuine Rust `fill_write_only` through normal KIR V9/LLVM lowering and
ROCm machine-code emission. Its exact function is 68 bytes and 14 instructions;
trailing NOPs are outside the function symbol. The new HSACO query derives the
scratch-free gfx942 input register locations from the selected inspected
descriptor. This is CPU-qualified descriptive layout, not a formal ABI proof,
machine-value relation or execution authority.

1. Reuse the existing same-owner ABI/descriptor transition. The normal target
   stage already retains semantic/KIR/formal ownership and typed roots; inert
   worker-handoff construction validates the descriptor and embeds it into LLVM.
   The plain LLVM capture stops before that construction. Do not copy those
   fields into another owner or treat standalone diagnostic LLVM as a protected
   handoff. Compose the existing canonical descriptor with physical inspection
   and generated packing for the exact selected artifact.
2. Check the complete real `fill_write_only` source/KIR chain: Index1d -> get ->
   u32 truncation -> guarded write through the original output binding. The
   retained ranked projection now records `ValueAccess`, and exact global-X KIR
   value normalization preserves the u64-to-u32 cast. The generic validator still
   excludes complete indexed-address/operational equivalence.
   Bind actual call spans, predicate `index < len`, output address and stored value,
   and reject all unsupported reachable effects. Use the distinct conditional
   coverage boundary and discharge `N <= G` from actual packed output length and
   AQL geometry; do not promote it into unconditional TotalView coverage. Bind
   the source-output ordinal through descriptor `SliceLengthU64`, never through
   the ranked extent ordinal. A bounded formal-memory witness alone does not
   establish full result initialization. Do not prioritize isolated helper proofs.
3. Prove the captured entry-to-exit machine relation. Inputs are kernarg `s[0:1]`,
   workgroup X `s2`, workitem X `v0`; the new layout query derives, not assumes,
   these locations. Cover the exact kernarg load, shift/OR index construction,
   wait, unsigned comparison/EXEC mask, scaled address, four-byte store and both
   paths to `S_ENDPGM`. In particular, the load captures its base before that pair
   is overwritten, OR equals addition only for local X below 64, and Y/Z geometry
   must not duplicate writes. Pointer validity, checked extent/address arithmetic,
   active lanes and memory visibility/completion remain separate obligations.
   The shared dispatch proof now constructs projected lane IDs and full EXEC
   from validated geometry and the selected descriptor profile. It proves
   parametric dispatch-wide unique coverage without grid-sized simulation or a
   caller-supplied activity mask. Authenticate the real entry/storage association
   and complete source-to-machine correspondence; the projected construction
   does not observe actual hardware registers or establish scheduling.
4. Qualify altered parameters/components, offsets/address spaces, owner/artifact
   substitution, changed result/literal, wrong store address/value, missing
   termination and extra effects. Only a complete proved profile and authenticated
   producer can satisfy the existing semantic-machine provider contract. Keep
   partial relations non-authoritative and the production admission gate closed.

Deployment-approved measurements, protected ledger access and rollback policy
remain separate deployment inputs. A complete semantic-machine verifier is
missing implementation, not a configuration toggle. Existing native peer,
adoption and issue witnesses do not substitute for it.

## Deferred Work

Do not place eight-device coverage, two-host distribution, broader collectives,
same-process device reopen, more benchmark wrappers or performance tuning ahead
of these functional paths. Post-arm hardware fault injection requires isolation;
retain CPU rejection/retention tests in the native packet now. Physical overlap,
matched HIP/HSA speedups and A3/A7 exits remain separate acceptance gates.
