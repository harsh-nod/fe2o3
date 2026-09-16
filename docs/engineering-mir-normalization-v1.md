# Engineering MIR normalization

`cargo fe2o3 engineering hsaco` accepts an explicit, closed normalization choice:

- `--mir-normalization minimal-v1` is the default: MIR inlining disabled, O0.
- `--mir-normalization optimized-inline-v1` enables rustc MIR inlining and O3.

Both retain `-Zalways-encode-mir`, disable JumpThreading, and use the exact target
CPU/features selected by `--target`. Arbitrary flags are not accepted through
this option. The engineering observation records the normalization name and the
complete extraction flag string separately from the LLVM worker's O2 setting.
Neither profile grants publication, load, launch, or production qualification.

## Why inlining is useful

The optimized profile exposes ordinary Rust pointer adapters and atomic RMW
wrappers to the existing intrinsic importer. It does not mark pointer-returning
calls pure or substitute handwritten Kernel IR. Every surviving operation must
still pass checked semantic import, allocation provenance, ranked bounds/effect
analysis, formal memory checks, and target-KIR lowering. Unsupported helpers and
operations continue to fail closed.

The collector separately audits every exact compiler-recorded inlined Instance
origin, including calls no longer present in executable MIR. Local HIR unsafe
blocks and unreviewed external helpers remain rejected. The audit graph has
bounded scope/function counts and is not added to the executable closure.

## Atomic scope

The source regression exercises u32 exchange/add/sub/and/or/xor/unsigned min/max
and i32 signed min/max with all five Rust orderings, global address space, and
system scope. The regression checks the emitted LLVM and ten formal accesses.
A separate ordinary-source regression exercises four u32/i32 load/store pairs,
covering relaxed, acquire/release, and sequentially consistent orderings. The
emitted gfx950 LLVM contains four loads and four stores with eight formal
accesses; every loaded SSA value feeds its corresponding store. No RMW
emulation or narrowed synchronization scope is used.

The pinned core load/store wrappers retain an ordering switch under this
profile. Their normalization requires authenticated core identity and exact
monomorphic signatures, a literal legal ordering, and the reviewed eight-block
MIR graph. Every legal arm must call the exact compiler intrinsic on the
original pointer/value and return its result unchanged. Extra legal-arm
statements, changed dataflow/orderings, alternative returns, and extra blocks
are rejected. Invalid-order arms must terminate in core panic; they cannot be
selected by an admitted call. The preflight recipe also binds the exact wrapper
MIR definition. A matching function name or a freshly observed digest alone
does not authorize normalization. Dynamic/invalid orderings and user-defined
lookalike wrappers have source rejection regressions.

Read-only raw pointer casts use Kernel IR V11's one-way pointer access
restriction, preserving the allocation, pointee, and address space. Neither
access widening nor pointer provenance reconstruction is admitted.

The current bound gfx950 capability contract still rejects 64-bit global
system-scope load/store; the regression preserves that rejection. Compare-
exchange, arbitrary widths, a persistent scheduler, and a GPU launch are not
qualified by these compiler tests. Atomic storage must satisfy the existing
device ABI's alignment, lifetime, coherence,
and no-conflicting-nonatomic-alias requirements. In particular, atomics do not
make an oversubscribed grid-wide spin barrier safe.
