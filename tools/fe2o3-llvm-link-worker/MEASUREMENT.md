# Worker trust boundary

`fe2o3-worker-v1-sha256-*` is an asserted build claim. CMake derives it from
the configured source files, compiler and linker configuration, LLVM package
version, and the caller-supplied LLVM build-ID file. It is not a cryptographic
measurement of the final worker executable or of the LLVM and LLD libraries
loaded at runtime.

The legacy pathname helper must therefore not be used as production execution
evidence. The authenticated physical machine-effect API instead pins the final
worker bytes in a sealed memfd, derives the executable identity from those
bytes, observes and retains the dynamic loader and every mapped DSO, and
revalidates their metadata and contents before input and after execution. Its
deployment policy pins that measured runtime-closure identity. The protocol
decoder, LLVM, LLD, worker, kernel analyzer, operating system, and retained
runtime files remain the trusted computing base.

Authenticated execution uses a fail-closed no-fork profile when no private
delegated cgroup is used: UID 0 and nonzero capability sets are rejected, the
child's hard `RLIMIT_NPROC` is zero before stdin, and the native entrypoint
installs the same limit before request collection. Memory, data, file, input,
output, metadata, symbol, and runtime-file collection are bounded. Process
groups are cleanup defense only.

Successful worker output is still inert. The worker verifies an exact AMDGPU
ELF/AMDHSA target contract, code-object ABI version, symbol closure, requested
exports, and the output digest. It does not publish or load the result.
`fe2o3-hsaco-finalize` remains responsible for final descriptor-table
inspection and finalization, and the transaction layer must pass all policy
gates before making the code object loadable.

Physical machine-effect evidence is limited to reachable static instruction
sites and bounded effect widths in exact `gfx942:xnack-` COV6 HSACO bytes. It
does not establish concrete addresses, runtime counts, OOB absence, race
freedom, compiler/source refinement, Verus correctness, or safe launch.
