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

## RMW scope

The source regression exercises u32 exchange/add/sub/and/or/xor/unsigned min/max
and i32 signed min/max with all five Rust orderings, global address space, and
system scope. The regression checks the emitted LLVM and ten formal accesses.
This is not a claim that atomic load/store, compare-exchange, every integer
width, a persistent scheduler, or a GPU launch has been qualified. Atomic
storage must satisfy the existing device ABI's alignment, lifetime, coherence,
and no-conflicting-nonatomic-alias requirements. In particular, atomics do not
make an oversubscribed grid-wide spin barrier safe.
