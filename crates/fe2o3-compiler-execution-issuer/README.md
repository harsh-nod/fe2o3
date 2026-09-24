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
capability, authenticated external-anchor endpoint and pidfd, twelve-entry
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
