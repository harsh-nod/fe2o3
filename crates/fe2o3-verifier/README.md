# fe2o3-verifier

`fe2o3-verifier` defines bounded records and process execution at the
verifier-driver boundary. It normalizes proof configuration, requested
properties, trusted items, tool identities, policy, canonical request bytes,
process output, and strict recorder results.

The crate does not invoke a shell. `CommandSpec` keeps the program and each
argument separate, and the bounded executor launches only the planned recorder
with an empty environment, null stdin, and fixed working directory. Tests use a
synthetic fixture recorder that never invokes its verifier or solver arguments;
its `proved` output is fixture data, not a Verus result.

`execute_authenticated_recorder` is the measured Linux path. It reads the
claimed verifier, claimed solver, and evidence recorder exactly once into
anonymous executable snapshots, computes SHA-256 over those exact bytes,
compares each measurement to the caller-selected policy, and seals each snapshot
against write, growth, and shrink. It launches only the recorder snapshot. The
canonical request is also sealed. The result file is anonymous and is sealed
immediately after the recorder exits. `execute_authenticated_verus` remains a
deprecated compatibility name and has the same recorder-only behavior.

`execute_authenticated_verus_v2` is a separate, narrowly executable Linux
x86_64 controller. It launches digest-pinned immutable solver and
Verus snapshots as two independent stages. Each stage must implement the V2
`PIDFD-NONCE2` `READY/START/RESULT/SEALED/DONE/ACK` protocol; stock Verus and
Z3 do not implement this protocol. The controller binds the canonical request,
exact source and named dependency blobs, reviewed process policy, launch challenge,
post-observation stage nonce, both executable images, bounded stdout/stderr,
and strict opaque result envelopes.

Each target is created with `clone3(CLONE_PIDFD)` behind an assembly child gate.
While the child is blocked before `execve`, the parent atomically retains its
pidfd, opens its procfs directory, and applies `PTRACE_SEIZE` with
`PTRACE_O_EXITKILL|PTRACE_O_TRACEEXEC`. Only then does it release the gate and
require the exact initial `PTRACE_EVENT_EXEC`. Each checkpoint uses
`PTRACE_INTERRUPT`, consumes only the exact `PTRACE_EVENT_STOP|SIGTRAP` status
through nonblocking `waitpid(__WALL)`, and confirms procfs state `t` and the exact
calling-thread tracer TID before and after observation. A target cannot cancel
this ptrace stop with `SIGCONT` or a timer; unexpected signal-delivery and later
exec stops fail closed. The final authenticated checkpoint is the only normal
detach point. The policy identity binds the exact ptrace requests, options,
events, and wait flags.

The pidfd remains the lifecycle identity and is used for signal preflight, kill,
exit detection, and reaping. The raw PID is used only where Linux's ptrace and
`waitpid(__WALL)` APIs require the pid of the still-unreaped pidfd-owned tracee;
there is no raw-PID signaling, process-group signaling, or PID-reuse fallback.
Pidfd events are typed: ptrace `CLD_TRAPPED` notifications are never accepted as
exit or confirmed cleanup. One deadline is created before spawn. The nonblocking
pre-exec status pipe, ptrace waits, protocol polling, observations, and target
exit use it. Pidfd signal permission is preflighted with signal zero.

Both checkpoints require one thread; exact UID, GID, supplementary-group, and
capability-bounding state; no inheritable, permitted, effective, or ambient
capabilities; `no_new_privs`; the expected seccomp mode and filter count; and
exact soft/hard address-space, data, file-size, and core limits. Every
file-backed mapping is measured while stopped using fixed 64 KiB streaming
buffers and reviewed per-file and aggregate bounds. For every file-backed
executable VMA, the controller reads the target's exact live bytes and compares
them byte-for-byte with the pinned backing-object slice, including zero fill
beyond EOF; a backing-file digest is not substituted for live text. The first
stable executable baseline binds normalized path/class, permissions, file
offset, mapping length, backing length/digest, live digest, and an explicit live
vDSO digest, and must match reviewed policy. The unreadable, kernel-emulated
x86_64 vsyscall VMA receives a typed exact-range marker. Every anonymous mapping's
range, class, permissions, and size is included in the ASLR-dependent checkpoint
identity. W+X mappings and process-visible shared-writable executable aliases are
rejected. Anonymous executable mappings are rejected except for the bounded
kernel vDSO and exact x86_64 vsyscall mappings encoded in policy. The complete
mapping, baseline, live executable-page, and security observations must remain
unchanged at `DONE`.

The controller reads `/proc/thread-self/status` before cloning and rejects root,
unequal real/effective/saved/filesystem UID or GID tuples, a root supplementary
group, active permitted/effective/inheritable/ambient capabilities, inherited
seccomp filters, nonzero securebits, and non-default or auto-reaping `SIGCHLD`
policy. A fixed x86_64 assembly launcher issues `clone3`; its child branch jumps
directly into the assembly trampoline, while only the parent branch returns to
Rust. The launcher and trampoline use prebuilt POD/C-string state, scalar loads,
branches, and direct Linux syscalls only. Debug and release disassembly tests
reject child-side calls, PLT, memcpy/memmove, allocator, panic, bounds-check, and
Rust runtime paths after `clone3`. The trampoline clears active and ambient
capabilities and installs a classic
BPF seccomp filter before `execve`. That filter kills x32 syscalls; x86_64
`clone`, `clone3`, `fork`, and `vfork`; credential and capability mutation;
namespace entry/creation; and later `prctl` calls. Verified single-thread state
therefore closes process and thread creation for this direct-stage protocol.
The policy identity includes canonical bytes of the complete BPF instruction
sequence, architecture and x32 constants, denied syscall numbers, the exact
linked launcher/trampoline bytes, live-page bounds, and bounded anonymous-executable
exceptions. All non-protocol descriptors remain close-on-exec.

Cleanup never performs a blocking wait after signal failure. It accepts only
terminal `CLD_EXITED`, `CLD_KILLED`, or `CLD_DUMPED` pidfd events, and uses
nonblocking `waitid(P_PIDFD)` plus pidfd `poll` under a separate 500 ms cleanup
deadline. A kill failure or cleanup timeout returns `TerminationUnconfirmed`;
`Drop` uses the same bounded procedure. `PTRACE_O_EXITKILL` also kills a tracee
if its controller thread exits unexpectedly. These rules bound userspace waiting
but cannot bound a kernel syscall that itself fails to return.

`READY` carries the launch challenge. Only after the first frozen observation
and an empty control queue does the controller generate the unpredictable
stage nonce and send `START`. After nonce-bound `RESULT`, the target is stopped
again, queued `DONE` is rejected, and the controller requires and reads the
immutable result seal before sending `SEALED`. A nonce-bound `DONE` is accepted
only afterward. The result is reread after `DONE`, so post-`DONE` mutation is
prevented by the retained memfd seals rather than detected by a timing race.

The non-`Clone` `AuthenticatedVerusExecutionReceiptV2` authenticates that both
direct process occurrences completed this controller protocol at the frozen
checkpoints and exposes each role's stable reviewed executable-baseline identity.
It does not claim exclusive measured-image execution between checkpoints:
executable bytes changed and restored, an RW-to-RX-to-RW transition, a mapping
created then removed, or a writable alias opened then closed entirely while the
target runs is outside the receipt. `PTRACE_O_TRACEEXEC` rejects a later exec that
remains observable, but this is not a claim against every transient same-process
substitution surface. The receipt also does not constrain protocol-descriptor
delegation or unrestricted IPC, prove bounded kernel/filesystem I/O, establish
that Verus invoked the solver, interpret either opaque result as a proof,
authenticate the external policy reviewer, or grant proof, publication,
module-load, or kernel-launch authority. A production increment still needs a
reviewed adapter that faithfully drives pinned stock Verus and its solver while
preserving these bindings.

The V2 fixture tests use the checked-in
`tests/fixtures/authenticated-verus-v2-closure-v1.txt` manifest. It pins the
debug/release fixture image, exact runtime-library paths, lengths and SHA-256
digests, exact solver/verus closure digests, normalized executable-baseline
digests and counts, live executable-byte totals, and reviewed vDSO digest for the
recorded x86_64 GNU host; tests do not derive policy by replaying a failed
controller observation. A host whose loader, runtime closure, executable
baseline, or vDSO differs must receive an explicitly reviewed manifest update
rather than silently recalibrating. Generic test runs leave the six tests that
execute this pinned closure ignored and continue to run the portable ELF,
debug-section, reproducibility, disassembly, source/dependency/executable
substitution, and fail-closed closure checks. The dedicated
`Authenticated Verus reviewed host` workflow targets only the self-hosted runner
label `fe2o3-verus-reviewed-host-v1` and runs
`scripts/ci-authenticated-verus-reviewed-host.sh` after trusted `main` pushes or
an explicit dispatch in the canonical `harsh-nod/fe2o3` repository. The
`powderluv/fe2o3` mirror must point to the identical commit and does not own a
second reviewed-host runner. The same script is the pre-push publication gate
for changes to this controller, fixture, policy, or toolchain.
It runs all 14 debug tests and the 13 applicable release tests with
`--include-ignored --test-threads=1`. Serial execution avoids cross-test
ptrace/process-scheduling interference. A missing reviewed runner leaves the
gate pending, and a pinned host that drifts fails rather than skipping or
recalibrating.

The workspace dev profile strips debuginfo only from `fe2o3-verifier`; independent
checkout builds confirm that package scope is sufficient, without changing other
workspace dev artifacts. The fixture regression parses raw ELF section metadata
and rejects `SHF_COMPRESSED`, `.zdebug_*`, every name containing a debug/DWARF/GDB/
STABS/CTF/BTF marker, legacy `.line`/`.mdebug`/`.pdr`, debug-link/alternate-link/
mini-debug delegation, and embedded checkout-root bytes. The only exception is one
`SHT_PROGBITS` `.debug_gdb_scripts` section with exact `ALLOC|MERGE|STRINGS` flags,
alignment and entry size 1, and the path-independent
`gdb_load_rust_pretty_printers.py` marker bytes. Actual GNU and gABI compressed
DWARF mutations and a bounded two-root Cargo probe enforce this policy; the probe
also compares an unrelated package against a no-profile control. Release profile
bytes and their pinned closure/baseline records are unaffected by this rule.

The authenticated recorder receives a fresh 256-bit challenge and canonical
invocation, policy, request, claimed-verifier, claimed-solver, and recorder
digests. Its strict legacy
`FE2O3-VERUS-AUTH-RESULT-V1` envelope must echo every binding before embedding a
canonical `FE2O3-VERIFIER-RESULT-V1` payload. A stale result therefore fails the
challenge check, while policy, input, or executable substitution fails its
specific digest check. The returned transcript binds the exact bounded stdout,
stderr, and result bytes and their SHA-256 digests. It does not show that the
recorder launched either claimed tool. A `proved` outcome means only that the
recorder reported `proved`.

`bind_authenticated_proof_executable_v1` is the fail-closed artifact bridge. It
recomputes the retained invocation, policy, request, payload, and complete
authenticated-envelope identities; reparses the exact sealed result; matches
an independent `ProofMatchPolicy`; and constructs the exact
`ProofExecutableBindingV1`. The resulting inert identity commits the challenge,
all three measured executable snapshots, stdout/stderr/result transcripts,
compiler and source semantics, finalized HSACO digest, target and code-object
version, ABI, launch contract, effects, and policy records. These joins do not
authenticate verifier or solver execution and grant no proof or runtime
authority.

The bridge offers two separate freshness APIs. The existing
`AuthenticatedExecutionFreshnessV1` remains a process-local convenience for
tests and short-lived tools. Callers that need durable replay tracking can use
`PersistentProofFreshnessLedgerV1` with
`bind_authenticated_proof_executable_persistent_v1`. That path completes every
recorder-report and executable-record check first, then durably consumes the
exact challenge, transcript, and sealed-result identities before returning the
inert
`PersistentlyFreshProofExecutableBindingV1`. This distinct, non-clone type
retains the receipt. Its canonical identity commits the underlying recorder-report
binding, consumed challenge/transcript/result tuple, random ledger namespace,
generation, and resulting state identity. A rejected report does not
consume freshness; an I/O failure after a durable intent may conservatively
consume it.

Persistent state and intent records use separate versioned domains, canonical
little-endian encodings, SHA-256 checksums and state identities, fixed-width
execution identities, a nonzero creation namespace, and explicit count and size
bounds. Decoding rejects unknown or zero identity fields, duplicate
challenge/transcript/result axes, noncanonical ordering, malformed lengths,
trailing bytes, discontinuous generations, and recovery intents from another
namespace.

Callers must use `create_new` exactly once and `open_existing` thereafter.
`open_existing` never synthesizes missing state, so deleting the state cannot
silently reset replay history; creation also refuses an existing lock anchor.
On Linux, the ledger opens its owner-controlled directory and fixed files with
`openat2`, rejecting symlinks and magic links. It retains the directory
descriptor, addresses records relative to it, requires private regular
single-link files, and revalidates object metadata. Each transaction opens a
fresh nofollow lock descriptor before taking a nonblocking exclusive `flock`.
The transaction records its creating PID, rejects mutation after `fork`, and a
fork child cannot unlock the parent's transaction when its inherited value is
dropped. Intent and state files are written to exclusive temporary files,
synced, atomically renamed, and followed by directory syncs.

Recovery is explicit: a clean or newly initialized state is accepted; a
canonical unpublished intent is discarded; a durable published intent is
applied to the preceding state or finalized against the already-installed next
state. Conflicting, ambiguous, malformed, oversized, or unexpected recovery
files fail closed. Once intent publication is durable, recovery never makes that
recorder-report identity replayable.

`reconcile_control_flow_source_v1` decodes the canonical frontend sidecar,
recomputes its span-independent CFG identity, and reconciles bounded loop and
integer-switch claims field by field. The derived functional-specification
identity composes the exact source bytes, CFG identity, claims, and an existing
functional-specification identity without changing the frozen proof wire
format. `bind_control_flow_proof_request_v1` requires bounds and functional
correctness in the exact sealed request.

`bind_authenticated_control_flow_executable_v1` can only extend an existing
`AuthenticatedProofExecutableBindingV1`. It rechecks the sealed request,
successful result properties, proof target, and finalized executable's source
contract identity before retaining the complete measured chain. The result
properties are recorder claims; this operation does not establish verifier or
solver execution. Source and request bindings are descriptive inputs; none of
these types grants proof, compiler, module-load, or kernel-launch authority.

Security-sensitive consumers that require durable freshness use
`bind_persistently_fresh_authenticated_control_flow_executable_v1`. Its input
and output types are distinct from the process-local path, and its identity
retains the ledger receipt through the control-flow binding. Neither path grants
compiler, module-load, or kernel-launch authority.

`PersistentlyFreshMultiKernelProofAdmissionV1` is the corresponding inert
multi-kernel evidence set. Its constructor consumes non-clone
`PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1` values; a
process-local control-flow binding cannot satisfy the API. The set is
canonicalized by kernel identity and requires one contiguous persistent-ledger
history, unique ledger generations, unique kernel identities, SHA-256 payload
identities, and exact agreement on the finalized executable digest, target,
code-object version, compiler, artifact producer, claimed verifier/solver and
executed recorder closure, proof configuration, verification model, exact recorder-policy digest,
and invocation timeout. The verifier and solver measurements are claimed
identities; only the recorder is known to have been launched.
ABI, launch, source, contract, request, proof, and freshness identities remain
per-kernel and are checked again by
`PersistentlyFreshKernelProofAdmissionRequestV1` when a kernel is selected.

The aggregate identity commits its domain and version, finalized executable,
code-object version, ledger namespace, canonical kernel order, each kernel's
source/contract/request/authenticated-proof/persistent-proof/control-flow
identities, and each receipt's generation and resulting ledger-state identity.
Receipt ancestry is validated before construction, so distinct generations from
divergent local ledger branches cannot be combined.
Both the request and aggregate have private fields and are non-clone. They are
evidence only: `grants_load_authority()` and `grants_launch_authority()` return
false. This API does not implement or satisfy a Worker V2 prerequisite
authenticator.

## Exact gfx942 alpha/zeta source-proof records

`Gfx942AlphaZetaProofInputV1` is a sealed identity for the bounded alpha/zeta
CPU/shared-body source-model profile. It is not a GPU-kernel or machine-code
proof. `AlphaZetaProofSourcesV1::discover_workspace` starts from the workspace
and example Cargo manifests. Within this fixed profile it follows path entries
from ordinary and workspace `dependencies`, rustc module-file rules,
literal `include!` in structurally parsed item/expression/statement/pattern/type
positions, and literal `#[path]`. It treats `cfg(test)` modules as disabled and
fails closed on every other module `cfg`, module `cfg_attr`, file-level
`cfg`/`cfg_attr`, ambiguous module candidate, nonliteral include, or include
token hidden in an opaque macro. The measured graph includes `Cargo.lock`,
toolchain and Cargo configuration, the ordinary Rust model and shared CPU body,
the permission model, the Verus harness, and the structurally reached
`fe2o3-contracts` files. Declared manifests are checked for missing, extra,
role-swapped, oversized, or mutated entries relative to that discovered graph;
unreachable files are not enumerated.

On Linux, descendant discovery uses
`openat2(BENEATH|NO_SYMLINKS|NO_MAGICLINKS|NO_XDEV)`. The absolute workspace
walk, every descendant directory, and every source file retain their canonical
parent descriptor, entry name, object descriptor, and device/inode/type
identity. The workspace root and descendants additionally bind full metadata;
regular files require one link and retain their exact bytes. Discovery reopens
every retained name relative to its retained parent before and after reads and
during final lease validation, rejecting ancestor, mount, rename, or file
replacement without binding mutable metadata of unrelated ancestor siblings.
File roles, paths, lengths, SHA-256 measurements, and dependency edges
contribute to separate source-tree and dependency-tree identities.

This bounded snapshot is intentionally not called a complete source or verifier
runtime closure. It does not evaluate arbitrary target/feature cfg expressions,
Cargo target/dev/build dependency tables, build scripts, procedural or
attribute macros, macro-generated module declarations, macro expansions that
synthesize include paths without an `include!` token, generated files, inherited
environment, `vstd`, `rust_verify`, Verus support resources, compiler shared
libraries, or solver resources. Opaque macro bodies are only scanned for the
specific fail-closed include/import token cases above; they are not expanded.
The trusted-item inventory is derived from retained Rust token streams reachable
from the proof harness. It detects `external_body`, `assume`, `admit`, trusted
attributes, and explicitly imported trusted APIs. External `vstd`/builtin
imports remain unmeasured runtime dependencies. The recorder does not consume
the retained snapshot or its environmental generation identity.
`validate_workspace` may rediscover files for diagnostics, but no authoritative
result can arise from that reread.

The sealed input also binds the proof target, typed ABI, effects and launch
identities, measured Verus and Z3 names, versions, executable and configuration
digests, model identity, five source-proof obligations, proof-set nonce, and
per-kernel proof nonce. Its canonical target joins `DeviceTargetV1`, the
artifact `TargetIdentity`, and publication `TargetIdentityV1` for exactly
`amdgcn-amd-amdhsa` / `gfx942:xnack-` / 64-bit little-endian AMD wave semantics.

`record_descriptive_alpha_zeta_execution_v1` is a test and diagnostics helper.
It accepts caller-assembled `ProofCapsuleV1::new_inert` values and a bounded
process-local replay ledger, so neither it nor `ReviewedAlphaZetaProofSetV1`
can satisfy an authoritative boundary. It remains useful for mutation tests over
dependency, property, tool, model, nonce, and freshness substitutions.

`record_inert_alpha_zeta_executable_evidence_v1` consumes a non-clone
`PersistentlyFreshProofExecutableBindingV1`, whose authenticated recorder output,
artifact match, and durable receipt have private construction. It remains inert:
the recorder did not receive the retained source snapshots, did not establish
that Verus or Z3 ran, and does not cover the compiler/verifier runtime closure.
The join rechecks the sealed input configuration, source-derived trusted-item
inventory, exact typed `gfx942:xnack-` target, COV6, full artifact proof target
including kernel ID, the manifest-derived alpha/zeta logical and export symbols,
ABI, effects, launch, tools, model, and the artifact V1 seven-property envelope.
Only the five source obligations are established by the Verus harness; the
additional memory-safety and launch-validity envelope entries remain recorder
claims. `InertAlphaZetaExecutableEvidenceSetV1` requires one contiguous durable ledger
lineage and rejects mixed set context, repeated proof-binding identity, repeated
review nonce, and repeated execution identities.

`ReviewedAlphaZetaProofSetV1` consumes one non-clone descriptive alpha record and one
non-clone zeta record. It rejects mixed proof-set nonces, source/dependency
trees, tools, models, review policies, ledger namespaces, and noncontiguous or
forked freshness histories. It also requires distinct per-kernel input, proof,
challenge, transcript, result, and persistent-binding identities. These are
review mechanics, not authentication. The reviewer policy and proof capsule are
caller-selected, and the public inert freshness constructor does not show that
the persistent bridge ran. Both record families report false for proof or launch
authority. The executable-evidence construction authenticates only the recorder
and durable lineage, not Verus, the solver, compiler refinement, or GPU execution.

The model proves bounds, natural-number address representability, explicit
input-initialization premises, injective exclusive output ownership, and exact
bounded-integer functional postconditions. It deliberately does not claim that
the integer adapters refine IEEE `f32`, that ghost allocation facts match live
arguments, or that rustc, LLVM, HSACO, and gfx942 execution refine the source
model.

## Trust boundary

- `VerifierPolicy` is caller-selected input, not a pinned trust root. The
  recorder API derives executable digests from sealed bytes and requires them
  to match that policy, but only launches the recorder. Names, versions,
  configuration digests, model, axiom policy, and timeout ceiling are committed
  by canonical policy bytes as caller claims.
- A `Proved` result is a recorder report, not evidence that Verus or a solver
  ran and not authority to load or launch a kernel. The
  artifact finalizer must reconstruct and match target, configuration, model,
  invocation, tool, property, and trusted-item identities.
- The parser accepts the recorder envelope, not unstructured Verus output. A
  future trust-rooted recorder would need to translate actual verifier and
  solver outcomes, inventory trusted escapes, and emit the envelope only after
  both tools terminate. The current API does not prove those steps occurred.
  Only a recorder exit code of zero can produce a parsed result.
- Correlation IDs prevent accidental request mixups. Authenticated recorder
  executions additionally use an OS-generated challenge to reject stale output.
- `AuthenticatedRecorderOutputV1` has private construction and exposes
  descriptive measurements and transcript bytes only. It authenticates the
  recorder execution, not claimed verifier or solver execution, and has no
  proof, runtime, module-load, kernel-launch, or compiler-refinement capability.
  `AuthenticatedVerusExecutionEvidenceV1` is a deprecated compatibility alias.
- `AuthenticatedVerusExecutionReceiptV2` has private construction and is not
  `Clone`. It authenticates two direct process occurrences under the V2
  protocol at frozen checkpoints, not exclusive measured-image execution,
  Verus proof semantics, or a Verus-created solver child. Its runtime-closure
  and stable executable-baseline allowlists and review digest are caller-provided,
  and it grants no proof, publication, module-load, or kernel-launch authority.
- `AuthenticatedProofExecutableBindingV1` is also evidence only. The legacy
  conversion and artifact-binding paths remain explicitly descriptive and
  cannot acquire authority by supplying unmeasured identities.
- `PersistentProofFreshnessLedgerV1` can only consume recorder-report identities
  projected as `AuthenticatedProofExecutionIdentityV1`; its records and receipts do not
  grant module-load, kernel-launch, compiler, or runtime authority.
- `PersistentlyFreshProofExecutableBindingV1` and
  `PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1` have private
  constructors. Their type distinction allows downstream code to reject
  process-local freshness without treating persistence as proof authority.
- `PersistentlyFreshMultiKernelProofAdmissionV1` rejects evidence from mixed
  ledger namespaces, repeated generations, repeated kernels, or mixed shared
  executable/tool identities. This is local persistent set consistency, not
  rollback resistance and not authority to authenticate, load, or launch a
  Worker V2 bundle.
- `ControlFlowSourceBindingV1` authenticates internal agreement among source
  bytes, CFG identity, and claims only. Compiler/MIR reconciliation remains a
  separate measured obligation, and only the private recorder-report/executable
  bridge can construct the final control-flow binding. That bridge is inert.

## Current limitations

There is no reviewed production Verus adapter, pinned execution-policy trust
root, signature or remote-attestation scheme, compiler-refinement proof, or GPU
runtime authority. V1 authentication is local and relative to caller-selected
policy, and only its recorder is launched. V2 additionally launches both
protocol tools and measures their file-backed runtime closures, frozen anonymous
mappings, live executable VMA bytes, backing-object agreement, and stable
executable baselines at both checkpoints, but its review digest remains
caller-provided and its two direct stages do not prove a real Verus-to-solver
invocation relationship. Both sealed paths currently require Linux
`memfd_create`, `fcntl` seals, ptrace/pidfd support, and `/proc`.

The general-GEMM V2 runner is narrower than the public authenticated V2
protocol. It accepts only a same-process `GeneralGemmVerusRuntimeClosureLeaseV2`
over the exact protected `/opt/fe2o3/verus-runtime-v2/<version>` closure. It
revalidates retained path edges, objects, inventories, and its mutation journal
before and after each process; executes retained `rust_verify`, Z3, toolchain,
and DSO objects through fixed descriptors; and supplies only immutable sealed
embedded proof inputs under the authenticated V2 resource bounds and shared
process-group deadline/output supervisor. Exact positive and expected-negative
outputs are required before the non-`Clone` schedule evidence is built. The
legacy launcher-path API remains fail-closed.

Stock `rust_verify` and Z3 do not implement the V2 nonce/control protocol, and
Verus must create its reviewed Z3 descendant. This runner therefore does not
produce `AuthenticatedVerusExecutionReceiptV2`, does not claim the V2 frozen
mapping/checkpoint properties, and grants no compiler, artifact, publication,
load, or launch authority. Authenticated KIR-to-model correspondence and emitted
machine refinement remain separate open obligations.

The control-flow binding does not yet prove optimized MIR or machine CFG
equivalence. It gives those later compiler-refinement checks a canonical exact
identity to match; the source sidecar alone remains non-authoritative.

The persistent ledger requires Linux `openat2`, `flock`, atomic same-directory
rename, and durable file and directory `fsync` semantics on an owner-controlled
local filesystem. The retained directory descriptor prevents live pathname
replacement from redirecting one ledger instance. The caller remains
responsible for provisioning the same trusted directory across restart; the
pathname itself is not an authenticated storage identity.

The namespace and chained state identities detect accidental substitution and
bind receipts to one ledger history, but they are not an external monotonic
counter or keyed storage authenticator. Restoring an older `ledger.state` file
or a complete directory snapshot can therefore restore older replay state.
Preventing rollback by an administrator, malicious same-UID process, VM
snapshot, or storage layer requires an external monotonic/version anchor, such
as trusted hardware or a remote append-only service, checked against the ledger
namespace, generation, and state identity. No distributed or network-filesystem
locking claim is made; a compromised kernel is also outside this local trust
boundary. The V1 ledger is bounded to 65,536 consumed recorder-report identities and fails
closed at capacity.

A V1 timeout kills and reaps the direct recorder child, but does not establish
a process group or forcibly terminate arbitrary descendants. V2 does not use
process groups. Its pidfd identifies and reaps exactly the atomically created
direct target, while pre-exec seccomp denial and verified single-thread state
prevent that target from creating threads or descendants. V2 does not sandbox
ordinary filesystem, network, or IPC syscalls or prove the meaning of opaque
results. The target may use unrestricted IPC, delegate protocol descriptors to a
pre-existing process, or arrange writable aliases outside its own visible map.
The checkpoints do not detect RW-to-RX-to-RW transitions, executable-page changes
that are restored, aliases opened and closed, mappings created and removed, or an
`exec` and return occurring wholly between observations. Hashing mapped files,
procfs records, and live executable pages uses synchronous kernel/filesystem I/O;
deadline checks occur between operations, but a blocking syscall is not forcibly
cancelled. Cleanup reports `TerminationUnconfirmed` after its userspace deadline,
but cannot terminate a task stuck indefinitely in uninterruptible kernel work.
The exact vsyscall mapping is unreadable and has only a typed marker, not a live
content hash. V2 trusts the Linux kernel, procfs, pidfd/seccomp/memfd
implementations, `/dev/urandom`, and the externally reviewed policy. Hostile
kernel behavior and transient or external same-UID interference remain outside
the claim.
The existing legacy `build_invocation_plan` and `execute_recorder` APIs still
accept caller-supplied tool identities and intentionally cannot construct
`AuthenticatedRecorderOutputV1` (or its deprecated
`AuthenticatedVerusExecutionEvidenceV1` alias).
