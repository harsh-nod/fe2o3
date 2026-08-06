# Worker trust boundary

`fe2o3-worker-v1-sha256-*` is an asserted build claim. CMake derives it from
the configured source files, compiler and linker configuration, LLVM package
version, and the caller-supplied LLVM build-ID file. It is not a cryptographic
measurement of the final worker executable or of the LLVM and LLD libraries
loaded at runtime.

The supervising process must pin the expected response claim and measure the
final worker executable bytes. Deployment must separately measure or otherwise
attest the matching LLVM/LLD package and operating environment. Those measured
artifacts, the protocol decoder, LLVM, LLD, and the worker are the trusted
computing base for native linking.

Successful worker output is still inert. The worker verifies an exact AMDGPU
ELF/AMDHSA target contract, code-object ABI version, symbol closure, requested
exports, and the output digest. It does not publish or load the result.
`fe2o3-hsaco-finalize` remains responsible for final descriptor-table
inspection and finalization, and the transaction layer must pass all policy
gates before making the code object loadable.
