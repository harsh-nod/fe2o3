# Bounded instruction programs inside Rust kernels

The experimental `amdgpu_ordered_program!` expression lets a kernel author
write an ordered sequence of integer instructions with explicit register roles.
It is a diagnostic authoring path, not a general assembly language or a new
production artifact-admission route.

```rust,ignore
use fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn choose_bits(mut output: DisjointSlice<u32>, a: u32, b: u32, mask: u32) {
    let selected = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33);
        in(34) = a; in(35) = b; in(36) = mask;
        xor(scratch, input0, input1);
        and(scratch, scratch, input2);
        xor(out, input1, scratch);
    };
    let index = thread::index_1d();
    if let Some(slot) = output.get_mut(index) {
        *slot = selected;
    }
}
```

The expression computes `b ^ ((a ^ b) & mask)`. Its three inputs are read-only;
only `scratch` and `out` are writable. The full compiler fixture, including
positive and negative variants, is
[ordered_program_v32.rs](../crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/ordered_program_v32.rs).
The snippet illustrates the API; qualification must use an actual retained
compiler invocation, not this Markdown block.

## Current contract

- Exactly one unconditional, acyclic, direct-root program occurrence.
- `gfx942:xnack-`, wave64, required and maximum workgroup `[64, 1, 1]`.
- One to sixteen `u32` steps: `mov`, wrapping `add`/`sub`, `and`, `or`, `xor`.
- Five distinct literal physical bindings in `v0..v63`.
- Scratch and output initially undefined; reads use the pre-instruction state.
  Output must be defined on exit.
- Exact order is retained, including overwritten writes and self-moves.
- No authored memory access, barriers, atomics, branches, labels, literals,
  matrix operations, hidden special-register writes or additional modifiers.

The frontend independently checks typed constants and actual source placement.
The macro's compile-time checks are not trusted as the only validation layer.
The marker panics if executed as an ordinary host function.

## Where compilation happens

```text
Rust and typed instruction macro
  -> retained semantic MIR32 and source occurrence
  -> semantic SSA
  -> diagnostic canonical KIR17
  -> AMDGPU LLVM IR with one constrained inline-assembly unit
  -> native LLVM instruction encoding and code-object inspection
```

This does not bypass LLVM IR. Surrounding Rust remains subject to normal
lowering; the authored sequence is represented as one constrained unit inside
LLVM IR. The native test checks exact instruction bytes, register operands,
ordering and descriptor capacity. It does not prove whole-kernel byte stability,
register lifetimes, protected publication or GPU execution.

The raw KIR17 decoder/CPU debugger is diagnostic-only. It does not authorize
resuming the production pipeline or loading the resulting code. The existing
production ownership, proof and artifact requirements remain separate gates.

## Format compatibility

Wire versions are distinct grammars, not a numeric feature ladder:

| Format | Meaning |
| --- | --- |
| MIR30, intrinsic87 | Published saturating arithmetic; no authoring call tail. |
| Diagnostic MIR31 | Fixed ordered pair; historical scalar87 and source tails. |
| Diagnostic MIR32 | Bounded ordered program89; historical scalar87 and source tails. |
| MIR33, intrinsic90 | Existing Wave64 shuffle profile. |
| MIR34, intrinsic91 | Standalone scalar authored instruction profile. |

The historical `SemanticGfx942Inline*V30` Rust type names do not identify the
current standalone wire version. Former experimental scalar-MIR30 bytes must
not be interpreted as published saturation. Fresh source terminal identifiers
138–143 keep scalar authoring disjoint from saturation and Wave64. Incompatible
feature families are rejected rather than promoted by choosing a larger number.

## Inspect declared instruction syntax

With the normal diagnostic exporter producing `program.kir` and a matching
one-workgroup (64x1x1) simulation request, the installed read-only inspector
offers either its unchanged JSON report or an optional text listing:

```sh
fe2o3-program-inspect program.kir request.json
fe2o3-program-inspect --text program.kir request.json
```

The compatibility example `inspect_diagnostic_ordered_program_v17` accepts
the same arguments. Both modes use the same canonical V17 admission and CPU
preflight, do not execute the kernel, and finish rendering within the same
8-KiB output buffer before writing stdout. Text rendering escapes every kernel
and function name, and refuses overflow without publishing a partial listing.

For the three-step declaration above, the instruction portion is:

```text
  00: v_xor_b32_e32 v32, v34, v35
  01: v_and_b32_e32 v32, v32, v36
  02: v_xor_b32_e32 v33, v35, v32
```

This is declared syntax, **not native disassembly**. Dead writes and self-moves
remain in order. The listing includes the typed canonical identity, actual
logical coordinate/SSA IDs and declared VGPR bindings; those bindings and their
high-water count are not physical values, final allocation or occupancy.
`NoMemory` describes the authored region, not surrounding Rust memory accesses.
The listing is a human-readable view, not a new import/resume format or source
authentication. Use JSON for existing machine consumers; its schema is unchanged.

## What the debugger shows

CPU execution treats the program as one logical operation. Recorded views can
show its declared instruction/register plan, lane-zero inputs before execution,
the result afterward, reverse-restored inputs and a repeated result. Exact
configuration, event and revision identities distinguish those checkpoints.

Declared VGPR names are not captured physical register contents. Unqueried
values, scratch intermediates, physical EXEC, per-instruction stops, allocator
lifetimes and occupancy remain unavailable. Reverse navigation does not restart
compilation or provide a mutable compiler snapshot.

## Starting from ordinary Rust

The checked local materialization planner recognizes an exact selected typed
XOR/AND/XOR bit-select graph with three inputs, one output and no escaping
intermediates. It renders the expression above with explicitly supplied source
names, preserving the baseline owner and source-byte commitment.

This is not yet general source replacement: source names and the proposed
enclosing range are caller assertions, not authenticated SSA-to-HIR bindings.
Applying a replacement needs a checked source boundary and fresh frontend
compilation. Assembly-to-arbitrary-Rust decompilation and lossless bidirectional
roundtripping are not claimed.

See [the implementation status](assembly-authoring-implementation-status.md)
for the remaining full milestones, including memory/synchronization, gfx950,
matrix operations, physical-resource views and production qualification.
