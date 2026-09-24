# Native Prepared Launch

`ProtectedIssuerSupervisorV2::prepare_launch` consumes a prepaid
`AcceptedCompilerExecutionHandoffV2` and returns move-only
`PreparedProtectedIssuerLaunchV2` plus an unreserved storage-growth receipt.
It materializes and revalidates inputs to the static launcher. It does **not**
create a process, establish child confinement, execute the issuer, publish
readiness, serve requests, run a protected proof, or qualify a GPU kernel.
M0-M7 and 47/47 remain incomplete.

The implementation is in
[`launch_v2.rs`](../crates/fe2o3-compiler-execution-supervisor/src/launch_v2.rs),
with native supervisor transfers in
[`authority_v2_launch.rs`](../crates/fe2o3-compiler-execution-supervisor/src/authority_v2_launch.rs),
finite manifest I/O in
[`launch_v2_io.rs`](../crates/fe2o3-compiler-execution-supervisor/src/launch_v2_io.rs),
and shared mechanical predicates in
[`launch_checks.rs`](../crates/fe2o3-compiler-execution-supervisor/src/launch_checks.rs).

## Exact Invariant

Preparation revalidates the native supervisor and accepted handoff, then clones
the exact admitted launcher, issuer, root, policy, signing key, anchor transport,
service peer and client pidfd. It constructs a fresh native service-launch
manifest from the accepted client and anchor-service identities and the pinned
native policy, seals that manifest as a native capability, and retains a
separately charged transfer of the same sealed object.

The prepared owner keeps the accepted handoff, launch capability, launcher and
issuer images, sealed static manifest, twelve ordered source descriptors and
three supervisor-side pipe readers. Source index and eventual destination are:

| Index / Destination | Role |
|---|---|
| 0 | stdin reader, with the newly created writer closed |
| 1 | stdout writer |
| 2 | stderr writer |
| 3 | service-owned issuer root |
| 4 | authenticated rustc service peer |
| 5 | exact live rustc client pidfd |
| 6 | native issuer policy capability |
| 7 | policy-bound signing-key image |
| 8 | native service-launch capability |
| 9 | readiness writer |
| 10 | provisioned external-anchor peer |
| 11 | exact external-anchor service pidfd |

The table describes future installation: preparation does not install these
destinations or the static launcher's source slots `200..211`. All retained
sources are CLOEXEC. Four independent nonblocking pipe pairs supply standard
streams and readiness; closing stdin's writer leaves seven retained endpoints.
Pipe checks require FIFO type, correct access, CLOEXEC and NONBLOCK, exclude
APPEND/ASYNC/DIRECT/O_PATH, and match each retained reader to its writer.
Readiness capacity must fit one complete readiness record.

Before returning, the complete prepared chain checks, in order:

1. Supervisor, accepted handoff, launch capability, and equality of the native
   capability's manifest with the accepted manifest.
2. Current supervisor PID and procfs start time against the static manifest.
3. Native program/root/policy/key/anchor transfers against their original
   owners, then service-peer and client-pidfd transfers against accepted custody.
4. The service-launch capability transfer against its original sealed object
   and bytes, followed by pipe pairs, individual ends and readiness capacity.
5. The exact source table and issuer snapshot, sealed static-manifest metadata
   and bytes, and non-aliasing of the launcher with every other launch role.

`revalidate` repeats this chain under the same caller ledger. These are ordered
observations, not an atomic global snapshot, perpetual liveness, or proof that
no other process holds a descriptor. No public prepared API exposes a descriptor,
signing operation, `Clone`, or conversion from an admitted V1 prepared owner.

## Bounded Transfers And Parent

Executable transfers must name the retained sealed object, not just an image
with equal bytes. Policy and launch-capability transfer checks compare the
original and borrowed image with fixed positional reads and pre/post metadata
checks. The signing-key transfer additionally requires the exact native policy,
secret-image owner/access rules and constant-time seed equality. Its temporary
seed storage is guarded for wiping; no new key derivation or signing occurs.
Secret-image transport is readable by its trusted holder, not a secrecy boundary
against that holder.

Root transfer repeats the shared root snapshot/security checks. Anchor transfer
uses native continuity checks over the original endpoint, live pidfd, target,
start time and credentials. Client-pidfd transfer validates original, duplicate,
then original again against the retained snapshot, with CLOEXEC checks on both
sides. Borrowed client-transfer validation uses a temporary duplicate, drops it
before return, and does not retain a second pidfd in the accepted owner. The
prepared source table deliberately retains its own separately charged duplicate.

`current_process_start_time_ticks_v2` uses the same ledger and bounded native
procfs inspection: fixed decimal path buffers, a 4097-byte record buffer,
at most 4097 read attempts, and a 4096-byte accepted stat-record limit.
EINTR is a refusal, not a retry. The `/proc/self` and numeric current-process
entries must agree on a procfs filesystem. A compatible trusted procfs mount is
still an environment assumption. The returned scalar and `getpid` establish
parent continuity when compared; they confer no process-launch authority.

## Shared Fixed Manifest

`StaticPreexecManifestV1` is the existing **inert structural codec**, not a V1
authority owner. Its wire remains the static launcher's 704-byte V1 format.
The value now stores a fixed 16-entry backing table and active count; native
preparation uses `from_descriptors` with twelve entries. Construction, decoding,
encoding and cloning of that record do not allocate. Native preparation never
uses the compatibility `new(..., Vec<...>)` constructor. There is one codec,
not a second native wire family or a V1-to-V2 admission adapter.

The codec checks structural bounds, source/destination order, object classes,
reserved bytes and alias rules. The shared launch predicates separately compare
the exact twelve expected destinations and observed source identities.
Pidfd-class tags are inert: actual target/start-time/liveness admission remains
with the native client and anchor owners. Reusing these data types and predicates
does not permit accepting a V1 program, policy, key, handoff or prepared owner.
Native refusal never falls back to V1.

The static manifest is an anonymous mode0400 memfd owned by the current effective
UID/GID, reopened read-only with CLOEXEC and exactly WRITE/GROW/SHRINK/SEAL seals.
Creation performs one 704-byte `pwrite`, one `fsync` and fixed seal/open operations.
The original object stays pinned while `/proc/self/fd` plus a bounded decimal
descriptor name opens and validates the read-only view. A short write fails.

Validation checks flags, access, seals, stat/owner/length and retained identity;
performs one 704-byte `pread` and one one-byte EOF probe; decodes and compares
both the typed record and canonical bytes; checks manifest-object non-aliasing;
then repeats metadata validation. Short reads, unexpected trailing bytes and
EINTR fail without retry. Legacy code uses the same metadata predicate, but its
existing read/write helpers are not the native finite-I/O path.

## Resource Contract

Every nested native operation uses the original caller ledger. The borrowed
supervisor and consumed accepted handoff must already be prepaid. Scopes restore
entry storage on success, refusal and unwind; charged work, peak storage and
denial history remain. On success, reserve the returned growth immediately while
preserving the consumed handoff reservation. On consuming refusal, descriptors
owned by the consumed handoff and staged result are dropped before the caller
retires that handoff's old reservation; the borrowed supervisor remains owned.

Let `Daccepted` be the consumed handoff's complete retained charge, `Dinputs`
the sum returned by supervisor-input transfers, `Dpeers` the service-peer/client
transfer charge, and `Dcap` the new launch capability's complete retained charge:

```text
Dinputs = launcher File charge + issuer File charge + ROOT_FILE_STORAGE
          + PolicyCapabilityV2::FILE_STORAGE + KeyCapabilityV2::FILE_STORAGE
          + AnchorV2::PAIR_STORAGE
Dpeers  = AcceptedV2::CONTROL_STORAGE + LiveClientV2::FD_STORAGE
growth  = Dinputs + Dpeers + Dcap + LaunchCapabilityV2::FILE_STORAGE
          + 7*PreparedV2::PIPE_STORAGE
          + PreparedV2::MANIFEST_FILE_STORAGE + PreparedV2::OWNER_GROWTH
Dprepared = Daccepted + growth
```

The fresh manifest's full charge plus capability-creation growth equals `Dcap`;
do not add that manifest charge twice. Executable File charges include their
declared image lengths. Duplicate descriptors conservatively pay complete logical
image charges even when they share an inode. Release `Dprepared`, not just growth,
only after dropping the prepared owner or correctly transferring its custody.

Current outer constants, with `B = 704`, are:

```text
PIPE_STORAGE          = size_of::<(OwnedFd, StorageV2)>()
MANIFEST_FILE_STORAGE = size_of::<(File, StorageV2)>() + B
OWNER_GROWTH          = size_of::<(StaticManifestV1, ObjectIdentityV1,
                                 usize, StorageV2)>() + align_of::<PreparedV2>()
WORK                  = 8 + 512*1024 + 64*B
SCRATCH               = 4*size_of::<(PreparedV2, StorageV2)>() + 8*B + 8192
```

Here `StorageV2` is `ProtectedIssuerLaunchStorageV2`. Fixed source/descriptor
arrays, codec buffers, temporary descriptors and control/error staging are
covered by the outer allowance. `WORK` conservatively covers the fixed utility
codec traversals and descriptor/pipe/manifest syscalls; the outer fixed-table
loops allocate no growable collections. Native executable validation has its
own image-size-dependent buffer/work quotas. Nested native operations charge
their own scratch on the same ledger. During preparation, each returned component's
growth is reserved before subsequent operations, and the final full check runs
with all prepared growth live. Revalidation requires
`supervisor.retained_storage() + prepared.retained_storage()` as its input floor.
The peak is the maximum simultaneously live owner/growth/outer/nested scratch,
not the sum of all sequential scratch allowances.

### Work Formula

For a successful complete call, define these work charges (each includes its
own nested operations, not merely its exported outer constant):

| Symbol | Operation |
|---|---|
| `O` | `PreparedV2::WORK`, currently 569352 |
| `S` | supervisor `revalidate` |
| `A` | accepted handoff `revalidate` |
| `Ic`, `Iv` | supervisor `clone_launch_inputs`, `revalidate_launch_inputs` |
| `Pc`, `Pv` | accepted `clone_launch_peers`, `revalidate_launch_peers` |
| `M` | native launch-manifest construction, currently 3592 |
| `C` | launch capability create/clone/revalidate/validate-transfer, each currently 36360 |
| `T` | `CURRENT_PROCESS_START_TIME_WORK_V2`, currently 9177128 |

```text
check      = S + A + Iv + Pv + T + 2*C
revalidate = O + check
prepare    = O + S + A + Ic + Pc + M + 2*C + T + check
```

Preparation calls its final private `check` without a second prepared outer
charge. Likewise, each input-transfer helper prepays `SupervisorV2::WORK` and
calls private `self.check`: together those cost one complete `S`, not `2*S`.
The three program transfer wrappers charge their own outer work, not complete
program revalidation again.

For an expansion into exported component quotas, let `G` be
`AdmittedIssuerProgramV2::WORK`, `H` accepted handoff `WORK`, `PolicyIO` the
policy capability `IO_WORK`, and `KeyIO` the signing-key `IO_WORK`. Let
`Lr`/`Ir` and `Lt`/`It` be native executable `quota(measurement, operation).work()`
for launcher/issuer under `Revalidate` and `Transfer`, respectively. Then:

```text
S  = SupervisorV2::WORK + G + PolicyIO + Lr + Ir + KeyIO
     + AnchorV2::REVALIDATION_WORK
A  = H + S + M + LiveClientV2::REVALIDATION_WORK
Ic = S + 3*G + Lt + It + PolicyIO + KeyIO + AnchorV2::CLONE_TRANSFER_WORK
Iv = S + 3*G + Lt + It + PolicyIO + KeyIO + AnchorV2::VALIDATE_TRANSFER_WORK
Pc = H + LiveClientV2::CLONE_TRANSFER_WORK
Pv = H + LiveClientV2::VALIDATE_TRANSFER_WORK
```

`M` is also the native manifest-policy comparison charge. Key transfer validation
uses `IO_WORK`, not admission/key-derivation work. Image cloning and borrowed-image
validation both select the executable `Transfer` quota. Thus the formulas depend
on the pinned image measurements; substituting just a supervisor or program
outer constant would undercount nested operations.

These formulas describe the current call graph, not elapsed time or an external
stable quota ABI. Constants may adjust during tests and review. Refusal charges
only reached scopes according to entry/prepayment order, with no refund. Logical
quotas do not bound generated instructions or stack, allocator behavior, RSS,
kernel objects/pipe buffers, page cache, syscall scheduling or wall-clock time.

## Remaining Integration

Native prepared custody now has a [consuming process lifecycle](compiler-execution-consuming-launch-v2.md)
with prepaid staging, gated profile checks, exact static exec and bounded pidfd
cleanup. Execution, validated readiness, publication and serving remain distinct
states. Isolated full native process validation and production integration are
still required. The existing V1 `run_session` is not a native prepared-state
consumer or a permitted fallback.

Native service handlers and durable-state families, independent broker V4
occurrence observation, anchor/Worker/issuer-ACK ordering, Cargo recovery and
restart paths, runtime/finalizer/host artifact joins and coherent provisioning
remain before producer activation. See the
[native publication contract](compiler-execution-publication-v2.md).

Source-level preparation, codec tests, exact/short accounting tests, or isolated
descriptor/process fixtures cannot stand in for production issuer execution,
protected-runtime proofs or GPU end-to-end qualification. This page records the
prepared-custody contract, not completion evidence for any milestone.
