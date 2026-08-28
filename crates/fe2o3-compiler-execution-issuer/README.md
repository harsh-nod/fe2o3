# fe2o3 compiler-execution issuer

This package is the descriptor-only entrypoint for the protected,
sealed-static compiler-execution issuer. It accepts no runtime configuration
from arguments or environment. A distinct-UID supervisor must launch it with
the exact fixed descriptor contract documented in the library.

The executable hardens itself before admitting key material, binds the sealed
launch manifest to the exact service peer and client pidfd, admits the
service-owned root and signing key, recovers both durable ledgers, and runs the
bounded compiler-execution service. It has no compiler, LLVM, linker, HSACO,
loader, KFD, or GPU dependency.

`scripts/build-static-compiler-execution-issuer.sh` builds the production
`x86_64-unknown-linux-musl` image and rejects an interpreter, dynamic section,
runtime dependency, executable stack, or undefined symbol. The repository
toolchain pins the required Rust target.
