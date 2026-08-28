# fe2o3 compiler-execution issuer

This package is the descriptor-only entrypoint for the protected,
sealed-static compiler-execution issuer. It accepts no runtime configuration
from arguments or environment. A distinct-UID supervisor must launch it with
the exact fixed descriptor contract documented in the library.

The production executable enters through a syscall-only shim before musl or
Rust startup. The shim immediately restores nondumpability after `exec`,
reasserts `no_new_privs` and the zero core limit, verifies the two process
controls, and only then transfers control to the normal static runtime. The
entrypoint then binds the sealed launch manifest to the exact service peer and client pidfd, admits the
service-owned root and signing key, recovers both durable ledgers, emits one
exact readiness record through a nonblocking atomic pipe, and runs the bounded
compiler-execution service. It has no compiler, LLVM, linker, HSACO, loader,
KFD, or GPU dependency.

`scripts/build-static-compiler-execution-issuer.sh` builds the production
`x86_64-unknown-linux-musl` image and requires the ELF entry address to equal
the secure shim symbol. It also rejects an interpreter, dynamic section,
runtime dependency, executable stack, or undefined symbol. The repository
toolchain pins the required Rust target. The pending protected supervisor must
still authenticate this complete image before placing the signing key in the
child descriptor table.
