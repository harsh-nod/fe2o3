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
and example Cargo manifests and recursively follows local Cargo dependencies,
Rust modules, `include!`, and `#[path]`. The resulting bounded, canonical
project-input snapshot includes `Cargo.lock`, toolchain and Cargo configuration,
the ordinary Rust model and shared CPU body, the axiom-free permission model,
the Verus harness, and the `fe2o3-contracts` manifest and source tree. Missing,
extra, role-swapped, oversized, symlinked-root, symlinked-parent, or structurally
ambiguous inputs are rejected. Discovery reads each file once and retains those
immutable bytes. File roles, paths, lengths, SHA-256 measurements, and dependency
edges contribute to separate source-tree and dependency-tree identities.

This bounded snapshot is intentionally not called a complete source or verifier
runtime closure. It does not measure Cargo build scripts, procedural macros,
`vstd`, `rust_verify`, Verus support resources, inherited environment, generated
files, compiler shared libraries, or solver resources. The trusted-item inventory
is derived from retained Rust token streams reachable from the proof harness. It
detects `external_body`, `assume`, `admit`, trusted attributes, and explicitly
imported trusted APIs. External `vstd`/builtin imports are retained as unmeasured
runtime dependencies. `validate_workspace` may rediscover files for diagnostics,
but no authoritative result can arise from that reread.

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

There is no reviewed production Verus recorder, pinned execution-policy trust
root, signature or remote-attestation scheme, dynamic-library closure
measurement, compiler-refinement proof, or GPU runtime authority. Authentication
is local and relative to caller-selected policy, and only the recorder is
launched. The sealed execution path currently requires Linux `memfd_create`,
`fcntl` seals, and `/proc/self/fd`.

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

A timeout kills and reaps the direct recorder child, but does not yet establish
a process group or forcibly terminate arbitrary descendants. The existing
legacy `build_invocation_plan` and `execute_recorder` APIs still accept
caller-supplied tool identities and intentionally cannot construct
`AuthenticatedRecorderOutputV1` (or its deprecated
`AuthenticatedVerusExecutionEvidenceV1` alias).
