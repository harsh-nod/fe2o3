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
uses a fresh-challenge ready/done/ack handshake to enumerate the exact
file-backed map set after loader initialization and again before permitting
exit. New mappings and `dlopen` fail closed. It revalidates retained metadata
and contents after execution, and its deployment policy pins the measured
runtime-closure identity. The protocol decoder, LLVM, LLD, worker, kernel
analyzer, operating system, and retained runtime files remain the trusted
computing base.

Authenticated execution uses a fail-closed no-fork profile. Real, effective,
saved, and filesystem UIDs must be equal and nonzero; the UID map must be the
initial full-range map; and inherited, permitted, effective, and ambient
capabilities must be zero. The child installs `PR_SET_NO_NEW_PRIVS`,
`RLIMIT_NPROC=0`, and hard memory, data, core, and file limits before
`exec`. The native entrypoint verifies this state, and a deployed test mode
attempts `fork`, `clone`, thread creation, and double-fork/`setsid` before
stdin. No cgroup-v2 or seccomp guarantee is asserted. Process groups and
descendant scans are cleanup defense only. Input, output, metadata, symbol,
and runtime-file collection are also bounded.

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
