# Assembly authoring: first implementation slice

Tracking: #280 (instructions/regions), #281 (resource views), #282 (multi-level
authoring). This is a prerequisite slice, not completion of those umbrellas.

## One program, several observations

The executable subject remains the ordinary source-to-MIR-to-canonical-KIR
pipeline. A simulation bundle is a bounded, content-bound observation of that
program, not a compiler resume token, source authenticator, or launch permit.
The new read-only authoring queries consume the exact Bundle V6 / KIR V11 body
and its embedded Source Map V2. They do not decode a second authoritative IR.

| Surface | This slice | Still unavailable |
| --- | --- | --- |
| Instruction contract | Existing gfx942 integer subset, shared operand/effect validation | Broad ISA, explicit allocation, scheduling and exact encodings |
| CPU execution | Six VGPR integer operations, exact 32-bit result bits | SGPR uniformity, EXEC/VCC/SCC state, instruction latency |
| Lowering | Existing LLVM inline-assembly path uses the shared contract | Direct assembler backend and final-code exact-contract checks |
| Debugger | Explicit `sim --bundle-v6` admission with embedded source binding | Live physical register/resource-state capture |
| Inspection | Bounded immutable source-bound snapshots and region boundaries | Mutable graph editor and production resume from snapshots |
| Source candidate | Representable u32 integer regions produce reviewable typed Rust | Production re-admission of those markers and automatic source replacement |
| Resource display | Exact stopped CPU checkpoint memory bytes and initialization | Allocation-lifetime inference, physical register values, GPU timings |

## Integer contract

The shared kernel-IR validator admits `v_mov_b32`, `s_mov_b32`, `v_add_u32`,
`v_sub_u32`, `v_and_b32`, `v_or_b32`, and `v_xor_b32` for the existing gfx942
lowering profile. Operand roles, register class, single i32/u32 result, exact
input types, complete source-identity shape, and NoMemory/effect-free contract
are checked. Nonzero source identities are not source authentication.

CPU simulation admits the six **vector** operations only. Add/sub wrap modulo
2^32; an i32 carrier preserves those same bits. The scalar-register move remains
unsupported because per-invocation scalar values do not establish SGPR uniformity.
The simulator retains existing instruction checkpoints and memory checks; it
does not emulate physical allocation or fabricate register observations.

Negative cases include wrong targets, operand roles/constraints/types, missing
source identity fields, declared memory effects, unsupported instructions,
stale snapshot selectors, oversized requests and inconsistent resource snapshots.
Existing surrounding KIR checks still own memory bounds/permissions,
initializedness, control flow, races and synchronization. Passing these checks
does not establish unmodeled hardware behavior or a universal refinement proof.

## Inspection command

After building `fe2o3-source-isa-observation --bin fe2o3-author`, pass exact
canonical Bundle V6 bytes on stdin:

```sh
fe2o3-author inspect < kernel-v6.fe2sim
fe2o3-author operations --bundle-identity HEX --start 0 --limit 16 < kernel-v6.fe2sim
fe2o3-author select --selector '{"bundle_identity":"HEX","canonical_kir_digest":"HEX","target":"gfx942:xnack-","operations":[{"function":0,"block":0,"operation":0}]}' < kernel-v6.fe2sim
fe2o3-author materialize --selector 'EXACT_SELECTOR_JSON' --helper compute < kernel-v6.fe2sim
```

`HEX` and operation coordinates must come from the actual snapshot. Ordinals
are local to that immutable snapshot, never persistent cross-build anchors.
Selections are contiguous in one block; live-in/live-out analysis includes the
containing function's uses and terminators. Queries and emitted source have
explicit bounds. Materialization returns JSON containing a diagnostic source
candidate, never edits a file, runs a compiler, or launches a kernel.

## The source-admission dependency

### Reproduce the supported ordinary-source path

Build `fe2o3-rustc-extract` and `fe2o3-export-sim` from `rustc-codegen-fe2o3`,
`fe2o3-author` from `fe2o3-source-isa-observation`, `fe2o3-kir-sim` from
`fe2o3-kir-sim-cli`, and `fe2o3-debug` from `fe2o3-debug-cli`, using the pinned
toolchain. Then run:

```sh
authoring_run=$(mktemp -d)
node scripts/authoring-v6-smoke.mjs "$authoring_run/capture"
```

The script exports actual Rust `examples/fill/src/lib.rs`, inspects/selects its
eight lowered operations, rejects a stale selector and unsupported promotion,
checks four independent 42.5f32 output words plus untouched canaries, and captures
a source-bound memory window through the V6 debugger route. It requires a fresh
output directory and retains its exact requests, responses and hashes.
`crates/fe2o3-source-isa-observation/tutorial/fill-v6` retains the actual exported
bundle for CLI regression tests; this is not a manufactured instruction fixture.

The companion site previews are `docs/inspect-lowered-kernels.md` and
`docs/resource-memory-windows.md` in `fe2o3-kernels`. They do not change existing
curriculum pins or claim that the blocked assembly source round-trip is shipped.

### Assembly source admission remains separate

The production terminal policy currently rejects `TrustedDeviceItem::AmdGpuInline`;
the semantic MIR compiler-intrinsic schema has no executable ISA variant.
Ordinary Rust `InlineAsm` is also rejected. Therefore this slice does **not**
promise that generated `amdgpu_asm!` Rust will compile through production.

Completing #280 source admission and #282 U2 needs a coordinated additive
semantic contract, authenticated importer expansion, ranked projection,
production KIR lowering, and fresh final-admission checks. The active #271/#272
owners retain their importer/lowerer integration boundaries. Replacing an
authored ISA instruction with an ordinary arithmetic operation would lose its
instruction-selection contract and is not an acceptable shortcut.

The eventual loop is source -> checked lowering -> explicit source promotion ->
fresh source compilation. Editing does not reuse prior proof receipts or captures.
Restoring retained high-level source is distinct from lifting edited assembly;
arbitrary assembly has no promised inverse into its original Rust.

## Independent remaining milestones

Complete assembly regions, explicit register/LDS ownership, waits/barriers,
target expansion and final-machine correspondence remain #280 work. Register/LDS
lifetimes, access views, tile/MFMA mappings and live GPU qualification remain
#281 work. Source application/conflict checks, production round-trips and checked
schedule recipes remain #282 work. A CPU replay schedule is not a compiler
schedule recipe. Tutorials must label each stage separately and must not replace
the existing source/evidence inventory or publication pins.
