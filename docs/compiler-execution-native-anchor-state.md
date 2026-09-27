# Native Durable Anchor State

## Status

`DurableExternalAnchorV2/V3` in `fe2o3-external-anchor-service` integrate native
sealed key custody with the same private durable-state engine used by V1. They
support initialization, strict reopen, atomic open-or-initialize, and signed
challenge exchange. They do not export keys or descriptors and do not upgrade
V1 owners. The state-file format and observation wire remain unchanged.

`serve_connected_peer_v2/v3` now drive those native owners through the same
peer I/O and scheduling implementation as V1, using the original resource ledger.
The dedicated native V2/V3 inherited daemon entrypoints now compose these APIs
with native process/namespace admission, measured sealed executable admission and
lifecycle custody. Dedicated native provisioning helpers now reissue native keys,
open-or-initialize state and exec the matching native daemon. The root coordinator
now has native preparation, revalidation, launch and managed lifetime. Native
supervisor transfer and compiler coordinator integration remain outstanding. Successful
protected startup and trusted parent provenance remain unvalidated. Neither these APIs nor their
tests complete M0-M7, prove kernel semantics, or qualify another end-to-end GPU
kernel.

## Invariants

The core owns the exact root descriptor and exclusive advisory lock, admitted
public key, cached sequence/head, and poison status. It never owns a signing key.
Each native wrapper owns its same-family key capability and borrows the actual
deployment on every operation. Complete deployment binding and exact nonroot
service UID/GID are checked before root mutation and again before response return.

An `Advance` from the exact prior position writes a fixed next-state image,
syncs it, atomically renames it, and syncs the directory before updating cached
state and signing. Exact retries at the proposed position do not rewrite state.
`Recover` observes only the exact prior or proposed position without advancing.

The core sets poison before the first persistence hook. Any persistence error
or unwind leaves it poisoned; reopening is mandatory, even if a rename occurred.
Malformed existing state is never reset by open-or-initialize. Only exact absence
permits genesis. The shared V1 path uses this same engine and poison behavior.

State reads/writes now make single attempts: short transfers and EINTR reject
rather than retry. Existing-state open uses NONBLOCK so a FIFO reaches metadata
rejection without blocking in open. Regular-file, ownership, mode, link-count,
exact length, key identity, checksum and framing checks remain in force. These
changes apply to V1 too; successful wire and state semantics are unchanged.

| Failure Point | State / Required Action |
| --- | --- |
| Input accounting, wrong context/key or credentials before persistence | No advance; consuming constructor errors close their inputs |
| Persistence error or unwind | Live core is poisoned; reopen and recover |
| Signing/key/resource failure after a completed persistence operation | No response returned; commit remains, so retry or reopen/recover |
| Invalid existing state at reopen | Reject; never silently reinitialize |

A constructor's late revalidation failure can follow genesis creation. Treat it
as a failed operation with possible persistent effects, not a rolled-back create.
Likewise, resource refusal is not a transaction rollback promise. Failure-injection
tests exercise the real I/O ordering; they are not power-loss tests or independent
verification of filesystem durability guarantees.

## Accounting

All operations use the caller's original `Budget::with_prepaid_scope`, including
nested key checks/signing. No new work ledger is introduced. Let `S` be
`NativeExternalAnchorStorageV2` (shared by both families), `K` the key's full
retained charge and `D` the borrowed deployment charge:

```text
R = size_of::<(OwnedFd, S)>()
A = size_of::<(DurableExternalAnchorVn, S)>() + KeyCapabilityVn::FILE_STORAGE
```

`A` conservatively includes the whole wrapper plus the sealed-image File charge.
Constructors consume prepaid `R + K`, borrow `D`, and return growth `A - R - K`.
Reserve that growth before retaining the owner; retire full `A` after drop.
On consuming failure, inputs are closed but their old reservations remain for
caller cleanup. Exchange borrows `A + D + 184` for an exact-sized challenge and
returns the full unreserved `size_of::<([u8; 288], S)>()` response charge. Wrong
lengths are not scanned and need no wire floor.

| Operation | Successful Work | Additional Peak |
| --- | ---: | --- |
| initialize / open / open_or_initialize | 244760 | `STATE_STORAGE + Key::IO_STORAGE` |
| exchange | 378656 | `STATE_STORAGE + Key::IO_STORAGE` |

Outer state work is 42504; construction additionally prepays 65536 for public-key
validation. Nested key operations charge the same ledger as they occur. Full
quotas are not reserved as one atomic transaction, so late budget refusal can
follow a durable commit. Entry work eight precedes input-floor checks. Every
scope restores entry storage on return/error/unwind without refunding work or
clearing historical peaks or first denials. `STATE_STORAGE` is the exported,
size-derived allowance for core, challenge, response, I/O and control staging.
Quotas are logical accounting, not syscall-time, generated-stack or RSS bounds.

See the [validation checkpoint](evidence/conditional-native-durable-anchor-20260926.md)
for source attribution, test failures, evidence hashes and remaining integration.

## Native Peer Loop

Each public native serving function borrows the anchor and actual same-family
deployment, consumes one prepaid peer descriptor, and uses the original caller
ledger. It revalidates key/deployment custody and exact service credentials before
transport validation. Public callers cannot supply a signer, persistence engine
or transport implementation, or convert a V1 owner into native authority.

The shared schedule is receive, durable exchange, send, retire response, increment
the checked exchange counter. V1 preserves its existing unmetered I/O behavior.
Native serving charges before every validation, poll, receive and send attempt,
including retries. The packet and response formats remain 184 and 288 bytes;
ancillary data and noncanonical packet lengths still reject. A response is
reserved immediately after exchange and remains charged through sending. Errors
and unwinds restore entry storage without refunding work or erasing history.

The input floor is `A + D + NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2`. The fixed
`NATIVE_EXTERNAL_ANCHOR_PEER_FRAME_STORAGE_V2` covers live packet/control staging;
peak additionally includes nested anchor/key scratch or the retained response.
The consumed descriptor closes on return/error/unwind. Its old reservation stays
for caller cleanup. Success returns the **full**, unreserved
`NATIVE_EXTERNAL_ANCHOR_PEER_REPORT_STORAGE_V2`, independent of that descriptor's
charge. Reserve the report charge before retention and retire it after drop.
The report is an inert V1 exchange counter, not an admitted V1 owner.

On Linux, logical work is:

| Stage | Charge |
| --- | ---: |
| Outer entry/frame | 31496 |
| Initial native key revalidation | 68360 |
| Endpoint validation | 13064 |
| Each poll attempt | 1032 |
| Each receive attempt | 6920 |
| Each durable exchange | 378656 |
| Each send attempt | 10248 |

With one successful poll per I/O, no retries, and EOF observed by a receive,
`n` exchanges cost `120872 + n * 397888`. Quotas bound logical attempts, **not**
idle poll time, syscall latency, filesystem durability or total service lifetime.
The existing 30-second response-publication timeout is unchanged.

Failure after persistence may leave a commit with no delivered response. Reopen
and recover/retry, never roll back or recreate state. Failure after send may mean
the peer already received the response. Peer shape and native custody are not
protected peer-identity, lifecycle or process-profile admission; startup callers
must establish those separately. No protected startup or compiler/GPU authority
is granted by this API or its tests. See the
[peer validation checkpoint](evidence/conditional-native-anchor-peer-20260926.md).

## Native Inherited Startup

`run_inherited_external_anchor_service_v2/v3` are unsafe, dedicated-process
boundaries. Call once in an isolated single-threaded daemon, with exclusively
transferred raw slots and no other live Rust descriptor owners or I/O threads.
Unrelated descriptors, including standard streams, are closed. Exit on failure;
never retry the entrypoint or return to an application that owns those descriptors.

| Slot | Transferred Input |
| --- | --- |
| 3 | Connected peer |
| 4 | Existing durable-state root |
| 5 | Root-owned lifecycle lease |
| 202 | Actual same-family policy |
| 220 | Actual same-family supervisor deployment |
| 221 | Actual same-family external-anchor deployment |
| 222 | Service-owned native sealed signing key |

Startup bounds argv0 inspection to 4096 bytes including its terminator, requires
one nonempty argument and an empty environment, then captures the nonroot process
and namespace profile. Descriptor cleanup happens before any descriptor-owning
native admission. The full source table is checked before duplication, so a
missing slot cannot be silently filled by an unrelated admitted capability.

Policy, supervisor and deployment are admitted in that order under their actual
contexts. Current credentials must match the deployment. The running sealed
executable is measured against that deployment before inspecting the key. Root,
peer and lifecycle inputs move to private CLOEXEC slots; context/key duplicates
remain owned through serving. Startup opens existing state only: it never creates
genesis or resets malformed state. Process, namespace, deployment and lifecycle
checks surround serving; the report remains charged through final checks.

`CompilerExecutionServiceLifecycleLeaseV2` shares the V1 filesystem/lock engine
but admits fresh custody on the original ledger. It additionally retains exact
parent device/inode identity. It rechecks the root-owned parent and canonical
root-owned lock file, including their required metadata and the shared
nonblocking lock. Drop only closes
descriptors, never explicitly unlocks a shared open-file description. There is no
public V1 upgrade, borrowed descriptor accessor or unmetered native operation.
Metered `try_clone_for_transfer` returns a controlled File sharing the owner's
open-file description. Reserve its full 16-byte charge on the tested x86-64 layout.
`validate_transfer` checks the exact canonical inode, parent, metadata and flags,
then acquires/rechecks the nonblocking shared lock. It does not prove that an
arbitrary candidate shares the original open-file description: an independent
reopen can acquire its own shared lock. Both operations cost 65544 work units.
Exported aliases are trusted transfer inputs, not safe for adversarial mutation of
flags or locks; drop only closes them.

Prepay the family's exported `STARTUP_INPUT_STORAGE` before entry (768 bytes on
the tested x86-64 layout). Every supplied slot is consumed/closed on return,
failure or unwind; the caller then retires its original input reservation. The
startup scope restores entry storage without refunding work or clearing prior
peaks/denials. Success returns the full unreserved peer-report charge, not growth
over any input. Reserve it before retention and retire it after drop.

Outer startup work is 262184 and its logical frame is 65536 bytes. Native nested
operations charge additionally on the same ledger. Lease admission/revalidation
cost 65544/32776 work units; admission returns growth over its consumed File,
whereas lease `open` and executable `admit_running` return full owner charges.
Running-image open makes one direct syscall attempt; EINTR rejects. Quotas are
logical accounting, not RSS, syscall latency or idle-service lifetime bounds.

The dedicated `fe2o3-external-anchor-service-v2` and `-v3` binaries create one
finite process ledger (work `1 << 40`, storage `1 << 30`). The static-image script
accepts explicit `v1`, `v2` or `v3`; omission preserves the existing V1 build. There
is no ambient family selector or native-to-V1 fallback. The root coordinator
still launches V1. See the
[startup checkpoint](evidence/conditional-native-anchor-startup-20260926.md) for
the initial daemon evidence and the following section for native helper integration.

## Native Provisioning Helper

`run_inherited_external_anchor_provisioning_helper_v2/v3` are unsafe, single-use
dedicated-process entrypoints with the same exclusive raw-slot and no-other-I/O
ownership requirements as daemon startup. Success replaces the process; every
return is terminal failure. Unrelated descriptors and standard streams close.

| Slot | Transferred Input |
| --- | --- |
| 3 | Connected root-parent bootstrap |
| 4 | Durable-state root |
| 5 | Measured sealed daemon image |
| 6 | Root-owned lifecycle lease |
| 202 | Actual same-family policy |
| 220 | Actual same-family supervisor deployment |
| 221 | Actual same-family anchor deployment |
| 222 | ROOT-owned native key template |
| 223 | Actual same-family provisioning context |

Invocation and process/namespace checks precede descriptor-owning admission. The
complete source table is checked before duplication. Actual policy, supervisor,
deployment and provisioning owners are admitted under their same-family contexts;
current nonroot credentials must match. The running helper is measured against
provisioning, the bootstrap must be an unnamed nonblocking UNIX seqpacket whose
peer is the exact root parent, and the daemon is measured against deployment.
Only then is the key template reissued to native service-owned custody. No raw
key is extracted and no V1 authority owner is admitted.

The helper opens or initializes state through the same native durable engine.
Malformed existing state still refuses. It retains the state lock, original
native owners and lifecycle lease while staging full File transfers above the
fixed table. Context, key, lease, image and profile checks run on those exact
transfers both before and after readiness send. Terminal exec inherits only the
daemon table `3/4/5/202/220/221/222` and an empty environment. Shared mechanical
descriptor/bootstrap/exec code preserves the legacy V1 path without introducing
a second persistence engine or a public test-provider interface.

Readiness is the existing 16-byte record plus one SCM_RIGHTS endpoint, **not**
proof that exec or daemon admission succeeded. Late refusal can follow genesis
creation or readiness delivery. Reopen and recover, never reset state or assume
rollback. The coordinator must still validate the readiness transfer, exec EOF,
live pidfd and protected endpoint identity before treating the service as ready.

Prepay the family's `NATIVE_EXTERNAL_ANCHOR_HELPER_INPUT_STORAGE` (134218656
bytes on the tested x86-64 layout). It conservatively includes the maximum
128-MiB incoming daemon image before its measurement is available. Intake and
staging separately reserve the overlap while two full charged images coexist.
Outer helper work is 401704 and its frame is 65536 bytes; nested native operations
charge the original ledger additionally. Mechanical I/O makes bounded single
attempts. Errors and unwinds close all inputs/private aliases, restore entry
storage and preserve work, peaks and denials; retire the original input charge
after return. Quotas are logical, not RSS, latency or syscall-duration bounds.

Dedicated helper V2/V3 binaries use one finite process ledger (work `1 << 40`,
storage `1 << 31`). The helper static-build script accepts `v1`, `v2` or `v3`,
defaulting to V1. Rootless tests validate refusal, restart and actual exec to a
minimal test image, not the production protected daemon. See the
[helper checkpoint](evidence/conditional-native-anchor-helper-20260926.md) for
exact passing/failing checks and remaining coordinator/compiler/GPU gates.

## Native Root Preparation

`PreparedExternalAnchorOccurrenceV2/V3` consume native deployment, provisioning,
root-owned key template and lifecycle custody, two source images and the state
root. Both borrow actual same-family policy and supervisor capabilities, not
just their identity fields. A single closed implementation checks complete
context binding, seals helper and daemon for the exact service, pins root
identity/metadata and records the preparing PID. Revalidation repeats context,
root, key, lease and image checks under exact root credentials.

The public APIs expose no V1 conversion, raw key, descriptor accessor or generic
provider. Preparation performs no persistence or process
creation and grants no compiler or GPU authority. Configuration custody does
not authenticate provisioning provenance.

`prepare_input_storage` is the full floor for every consumed owner, both full
source images and the two borrowed contexts. `preparation_quota` includes all
nested checks and the additional peak while image growth overlaps other scratch.
Reserve returned `GROWTH_STORAGE` before retaining the result; keep consumed
input reservations. After dropping the result, retire its full
`retained_storage`, which excludes borrowed contexts. On consuming failure,
owned inputs close but their reservations remain for caller retirement.

The local preparation/revalidation charge is 65544 units, excluding nested native
operations. `ROOT_STORAGE` is 16 bytes on the tested x86-64 layout. Frames and
envelopes are size-derived. Scopes preserve the original ledger's accepted work,
peak and first denials and restore entry storage on all exits. These are logical
quotas, not generated-stack, RSS, instruction-count or time bounds.

The existing funded cleanup pool has moved to `fe2o3-protected-service-spawn`
without a second pool or fresh ledger; issuer APIs retain their old names as
aliases. Root child setup rearms and checks the parent-death guard after the
credential transition, and uses shared bounded gate and capability-ceiling
readers. The legacy root child still synchronously reaps on Drop. See the
[root preparation checkpoint](evidence/conditional-native-root-preparation-20260926.md)
for preparation evidence; the next section describes the newer spawn primitives.

## Native Root Spawn Mechanics

The shared spawn crate now provides `StagedProtectedServiceExecV2` and
`RootOwnedProtectedServiceChildV2` through an explicitly unsafe mechanical bridge.
It does not turn raw inputs, V1 owners or storage numbers into native deployment
authority. The trusted caller must derive full source charges, retain native owners,
and validate every final staged File against the actual native owners and context.
Staging charges the full duplicate owner, including overlapping image bytes;
returned storage is a full unreserved charge, not growth over borrowed sources.

The original parent ledger prepays bounded child work before clone. Cleanup
capacity and the artifact lease are reserved first; the atomic pidfd, reservation
and lease enter a guard before any fallible parent check. Cancellation and Drop
take one prepaid cleanup step and transfer unresolved custody to the same global
pool. There is no native blocking wait, raw-PID kill fallback or new budget.
Missing pidfds and lost wait ownership quarantine custody, not successful reaping.
Verified exec releases only the spawn lease; it does not retire child ownership.
Deferred or quarantined leases can indefinitely delay artifact-lock descriptor
release. Cleanup must not wait for that release while retaining its own lease;
finite funding does not guarantee eventual reaping.

For `n` descriptors and capability ceiling `c`, child setup costs
`(166 + n + 3 * (c + 1)) * 1088 + 256` logical units, for `1 <= n <= 32`
and `0 <= c <= 63`. Its maximum is 424576. The maximum full spawn work is
529400, including parent work and cleanup reservation; staging costs 139672.
These bounds exclude the executed program and coordinator readiness. They are
not instruction-count, syscall/mutex latency, generated-stack or RSS bounds.

The root coordinator now integrates these primitives as described below. Rootless
atomic-clone/cleanup probes do not establish a successful protected credential
transition or helper startup. See the historical
[spawn checkpoint](evidence/conditional-native-root-spawn-20260926.md) for exact
primitive test results.

## Native Root Launch

`PreparedExternalAnchorOccurrenceV2/V3::launch` consumes preparation while
borrowing actual same-family policy and supervisor capabilities, the funded
cleanup controller and the original request ledger. `launch_input_storage`
includes those borrowed contexts; `launch_quota` includes every finite readiness
attempt, nested native operation and full overlapping image charge. It is a
conservative logical envelope, not a time, stack or RSS promise.

Preparation and revalidation now join the lifecycle lease to the actual state
root. Before the first child, `retain_cleanup_guard` installs a controlled alias
in the original empty cleanup pool. Launch validates that installed guard against
the retained lease before staging or spawning. Guard installation and cloning
charge both original accounts; neither operation replaces custody or resets
funding. An independently valid but unrelated lease or guard is insufficient.

The helper descriptor table is exactly `3, 4, 5, 6, 202, 220, 221, 222, 223`,
checked against the inherited helper ABI at compile time. The coordinator checks
the final helper/daemon Files, lifecycle lock, policy, supervisor, deployment,
provisioning, signing-key template, pinned root and private bootstrap identity.
Only then may the shared primitive create the guarded child. Namespace/profile
observation and input revalidation precede gate release. The same readiness
engine serves V1 and native callers, with native attempt/liveness charges on
the original account. It now lives in the shared spawn crate, with only nominal
anchor decoding in this coordinator. It has 120001 attempts per polling phase and 64 gate-write
attempts, plus one shared deadline of at most 120 seconds.

Ready transfer requires the exact canonical record and one CLOEXEC descriptor.
The receiver owns all disclosed SCM_RIGHTS/SCM_PIDFD descriptors before any
fallible check; excess rights, unknown control and truncation are rejected with
cleanup. Even successful I/O must meet the deadline. Exec EOF, continued exact
child liveness and native protected endpoint admission precede artifact-lease
confirmation and a managed result.

Terminal status enables `SO_PASSCRED` before receiving, using kernel credentials
only to distinguish records, including queued empty records, from actual EOF.
They grant no service identity. An unsupported or denied option refuses without
a weaker fallback. Live GNU/musl readiness-transfer tests pass, but terminal
credential tests encounter local EPERM. See the
[guard and transport checkpoint](evidence/conditional-native-root-launch-transport-20260926.md).

`RootManagedExternalAnchorV2/V3` retain preparation, endpoint admission and child
custody. Returned launch storage is growth above the consumed prepared owner,
not its full retained charge. Continuity rechecks actual contexts and retained
inputs. Cancellation/Drop use the prepaid finite child cleanup path; pending or
quarantined custody is not successful termination. Callers retire full retained
storage only after dropping the managed owner. Deferred artifact leases may
delay lock release indefinitely.

Native supervisor transfer now requires retained managed custody and actual
same-family supervisor/policy capabilities. `try_clone_for_supervisor` charges
the full new pair and nominal envelope; `into_ordered_descriptors` consumes that
envelope without silently retiring reservations. Final staged Files must pass
`validate_supervisor_transfer` against the retained managed owner and contexts.
No independent pair constructor or V1 upgrade is exposed. Budget/context and
descriptor-packaging tests do not prove a successful protected transfer.

The supervisor's policy-neutral `ProvisionedProtectedIssuerServiceInputsV2`
likewise admits, pins, clones and validates final listener/root objects under
the original ledger using shared filesystem/socket checks. Only the fixed
production pathname is public. Activation permits continuity but ends transfer.
Compiler coordinator native preparation now owns the genuine matching trust,
three freshly sealed images, listener/root, two root-bound lifecycle leases and
managed anchor. Lease validity is joined to the actual retained service root,
not accepted as an unrelated valid lock. Preparation itself spawns nothing;
the installed root entrypoint still uses V1 and remains to be migrated.
Consuming native supervisor launch now exists as described below. Dedicated
native V2/V3 supervisor binaries now
compose inherited descriptor intake, actual admission, listener activation,
bounded readiness and dispatch. Their protected deployment is not yet validated.
Startup transfers an actual lifecycle-lock alias into the existing charged
cleanup pool before launch; local errors cannot retire this guard while child
custody remains. See the
[startup checkpoint](evidence/conditional-native-supervisor-startup-20260926.md).
Current tests cover actual staged Files and rootless transport/refusal mechanics,
not a successful native root deployment. Adapter-level post-clone failures and
unwinds, including child-storage refusal and failure after endpoint receipt,
still need direct coverage. See the
[preparation checkpoint](evidence/conditional-native-compiler-preparation-20260926.md).

The shared spawn path now supports ordered deferred custody. Before clone, the
existing cleanup slot retains the complete dependency payload and charges its
full storage to the original persistent account. Pending or quarantined children
keep that payload; exact terminal wait retires it outside pool locks, permitting
nested anchor cancellation in the same pool. Metered exclusive access supports
the actual prepared owners without requiring their listeners to be `Sync`.
See the [retained-custody checkpoint](evidence/conditional-native-retained-custody-20260926.md).

Consuming supervisor launch now transfers its actual complete prepared owner,
live anchor and both leases through that path. It joins the installed cleanup
guard to the actual compiler lifecycle/root and validates final staged Files,
profile, nominal Ready, EOF, final context and child liveness before confirming
exec. Managed continuity retains the same owners and original request ledger.

This composition is implemented but not validated as a protected deployment.
Genuine-preparation quota boundaries, complete-path staged substitutions and
post-spawn refusal/unwind schedules still need coverage. Native inherited root
descriptor admission and anchor-first composition now exist, with bounded source
preflight, contextual records, zeroizing seed reads and shared listener mechanics.
Admission quota queries now cover the complete inherited admission at fixed
image ceilings, using the same executable cost calculation without constructing
placeholder authority. Bounded native activation mechanics are compiled and
component-tested but not installed. Complete launch/cleanup funding, matching
V3 provisioning, runner integration and protected validation remain outstanding;
see the [startup checkpoint](evidence/conditional-native-startup-bounds-20260926.md).
Neither `cancel`, field-drop order, a
persistent guard nor passing component tests establish successful production
startup. See the [compiler launch checkpoint](evidence/conditional-native-compiler-launch-20260926.md).
