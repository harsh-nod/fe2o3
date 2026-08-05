# AMDGPU G1 lowering

`lower_kernel_to_llvm_ir` is a target-neutral experiment, not the production
fe2o3 emitter. It verifies the complete kernel IR module before selecting one
exact `KernelId`, then performs a fail-closed AMDGPU-specific preflight before
emitting text.

G1 accepts:

- a 1D launch domain with either a static or dynamic total extent;
- a required static workgroup size `(x, 1, 1)`;
- scalar and global slice kernel parameters;
- global-X invocation index, slice length/data, constants, selected scalar
  arithmetic and casts, integer comparisons, GEP, aligned global load/store,
  and optional volatile memory access;
- conditional and ordinary branches without block arguments, void return,
  and unreachable.

The global index is computed in `i64` from AMDGPU workitem and workgroup IDs
using the declared workgroup X size. The total launch extent remains a host
launch contract and is intentionally absent from the LLVM IR. G1 does not add
`inbounds` to GEPs because the source IR does not carry that assertion.

G1 rejects declarations, non-void entries, 2D/3D domains, missing workgroup
sizes, capabilities, unsafe unquoted symbols, block arguments, non-global
memory, unsupported scalar types, and every operation not explicitly listed
above. It does not select a GPU processor, invoke LLVM, produce an artifact,
or grant launch authority.
