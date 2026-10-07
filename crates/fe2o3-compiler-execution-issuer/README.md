# fe2o3 compiler-execution issuer

This package is the descriptor-only entrypoint for the protected,
sealed-static compiler-execution issuer. It accepts no runtime configuration
from arguments or environment. A distinct-UID supervisor must launch it with
the exact fixed descriptor contract documented in the library.

The production executable enters through a syscall-only shim before musl or
Rust startup. The shim immediately restores nondumpability after `exec`,
reasserts `no_new_privs` and the zero core limit, verifies the two process
controls, and only then transfers control to the normal static runtime. The
entrypoint then binds the sealed launch manifest to the exact rustc service
peer and client pidfd, admits the service-owned root and signing key, and
independently admits the supervisor-provisioned external-anchor endpoint at FD
10 against its pidfd at FD 11 and the manifest-pinned service UID/GID. It binds
that transport to the policy's distinct external-anchor verification key,
and requires the root-created observer endpoint at FD 12 and original root
pidfd at FD 13. The authenticated greeting binds this actual issuer and its
supervisor to the exact launch before recovery or readiness. The empty-capability
issuer keeps remote occurrence guards and original publication-lock aliases
through signing and durable commit; it cannot fall back to local inspection.
After observer admission it
recovers both durable ledgers, emits one exact readiness record through a
nonblocking atomic pipe, and runs the bounded
compiler-execution service. It has no compiler, LLVM, linker, HSACO, loader,
KFD, or GPU dependency.

`scripts/build-static-compiler-execution-issuer.sh` builds the production
`x86_64-unknown-linux-musl` image and requires the ELF entry address to equal
the secure shim symbol. It also rejects an interpreter, dynamic section,
runtime dependency, executable stack, or undefined symbol. The repository
toolchain pins the required Rust target. The protected supervisor binds the
already authenticated launcher and issuer-program images to the exact
credential profile, protected root, caller policy, canonical signing-key
capability, authenticated external-anchor and observer endpoints/pidfds, fourteen-entry
descriptor manifest, and child lifecycle before launch. Publication does not
yet perform the external anchor exchange.

## Native Input Integration

`CompilerExecutionIssuerLaunchInputsV2::from_inherited` separately admits native
policy and launch-manifest capabilities from fixed slots 6 and 8, retains private
CLOEXEC duplicates, and requires their policy identities to match. Its nested
operations share one resource ledger. It never upgrades or retries a V1 policy.
The launch manifest retains the identity-only V1 wire; decoding that frame alone
does not establish a native policy match.

`run_inherited_compiler_execution_issuer_v2` consumes these inputs through native
client/service/anchor/key and running-image admission on one original budget.
Its separate `fe2o3-compiler-execution-issuer-native` binary uses the same fixed
descriptor ABI. The default executable and V1 entrypoint are unchanged; neither
wire decoding nor admission failure selects or retries the other version.

Native readiness is published only after singleton journal recovery, retained
anchor admission, exact manifest/client/policy joins and fresh custody checks.
The consumed pipe writer is closed after its single bounded atomic write. The
same native service then handles requests, including acknowledged cancellation
through `CompilerExecutionClientV2::cancel`. Readiness is not compiler-origin
proof, a successful anchor observation, or GPU authority. Prepare/Issue still
require independent pidfd inspection; permission denial is terminal.

The native binary still needs a separately measured sealed-static build and
trusted supervisor provisioning. The existing static build/deployment scripts
remain V1. An opt-in isolated static test stages public admission/readiness and
Cancel, plus refusal cases; it is not evidence of inherited-entrypoint launch,
protected compiler-source observation, proof execution, or GPU qualification.
See the
[native capability contract](../../docs/compiler-execution-capabilities-v2.md).

## Isolated Native Harness

The `native_admission_v2` and `native_admission_v3` integration-test targets share
the same fixture and case bodies. Each binds its own nominal policy, signing key,
issuer admission/error, launch manifest, readiness and wire-client types. The V3
client exchanges actual V3 Cancel/Cancelled packets; no V2 fixture is converted.
The V2 test names and success markers are unchanged. V3 markers use
`FE2O3_NATIVE_ISSUER_V3_PUBLIC_*` and `FE2O3_NATIVE_SERVICE_V3_READINESS_*`.

All four tests in each target are ignored by default. An ignored/default test
result is not qualification. Run only `isolated_static_public_admission_matrix`
or `isolated_static_public_native_service_matrix`, one exact test at a time with
`--exact --ignored --nocapture --test-threads=1`, in a disposable root Docker
container with `FE2O3_RUN_NATIVE_ISSUER_ADMISSION=1` and a separately built pinned
musl-static test executable. The other two tests are private subprocess roles.
A missing opt-in, wrong UID/GID, dynamic image or failed syscall is a failure,
not a successful skip. The runner must enforce a read-only root/executable,
private `/tmp` and namespaces, no network/GPU, default seccomp, no-new-privileges,
drop-all capabilities plus CHOWN/KILL/SETUID/SETGID, CPU/memory/PID limits and a
600-second outer timeout. The opt-in and Docker marker alone do not verify all
of these runner constraints.

Each family has 17 admission cases and six readiness/Cancel cases. Unchanged
bounds include 60 seconds per case, a 570-second admission-matrix deadline,
15-second IPC waits, 120-second peer waits, 24,000 polling attempts with 5-ms
steps, and 5-second child cleanup waits. Each work ledger allows 10^12 units;
admission storage is 512 MiB and service/client storage is 256 MiB. Fixture
management remains outside logical accounting. Issuer operations retain the
original ledger; the client role and deliberate foreign-ledger probes have
separate ledgers. Cleanup kills/reaps owned children and removes only the owned
empty root or its known journal file, never a recursive directory tree.

This harness covers public admission, readiness after singleton recovery, and
acknowledged Cancel only. It does not launch the fixed-FD issuer entrypoint,
install a trusted supervisor, independently observe a V5 Prepare/Issue, perform
an authenticated external-anchor exchange, or establish protected compiler/GPU
authority. Permission refusals remain terminal; there is no fake admission.

## Conditional Native Issuer

`run_inherited_compiler_execution_issuer_v3` requires FD3..12, with the original
root's private control endpoint at FD12. It preflights the complete non-CLOEXEC
table before allocating or duplicating descriptors. All V3 inherited duplicates,
including the sealed policy and manifest imports, start at FD13. V1/V2 retain
their FD3..11 contract and floor-12 consuming intake.

V3 shares process hardening, cumulative resource accounting, the packet loop and
durable recovery with actual V3 policy, signing-key, packet and journal owners.
`CompilerExecutionIssuerLaunchInputsV3` retains and revalidates both sealed input
objects; the identity-only manifest must match the independently decoded V3 policy.
Its pair-only API borrows FD6/8 and does not require the remaining service slots.
There is no wire sniffing, policy conversion, or fallback to the V1/V2 entrypoints.
The separate `fe2o3-compiler-execution-issuer-conditional` binary calls this entrypoint;
it does not change the default executable or trusted deployment.

The V3 entrypoint consumes FD12 into `serve_native_with_root_readiness`: endpoint
admission precedes readiness, and the root handshake gates the client packet loop.
The endpoint remains owned for the serving lifetime. Launch integration must stage
this same ABI on every V3 route, or refuse unsupported routes before child creation;
the indirect supervisor's own FD12 lifecycle lock is not the issuer root endpoint.

Prepare and Issue use an independently observed compiler process and retain its
exact V5 publication lease and consumption token. The published invocation must
equal the observed invocation before a SubjectV3 is derived. Fresh observation,
publication-lock and admitted-custody checks surround signing and durable commit.
Publication and currentness still require the independently signed anchor and
exact Worker/issuer joins. Readiness alone grants none of those claims.

Both native families use the existing singleton and journal filenames. Conditional
issuer/Worker records have distinct V4 framing and signature/identity domains;
their anchor journal is V3. A foreign or mixed-family record is a refusal, never
an empty directory, an automatic migration, or a reason to start another ledger.

This is issuer-side integration, not a completed production-to-GPU path. A pinned
sealed-static conditional image, trusted supervisor/launcher wiring, early
compiler-side custody on the original resource account, the real conditional
publication continuation, applicable machine-refinement proofs, and target-matched
end-to-end runs remain required. Component tests do not establish protected
execution, deployment provenance, machine equivalence, or 47/47 kernel coverage.
