# Scalar GEMM V1
This document specifies the retained scalar reference model and host-admission
contract. The retired scalar-specific Worker V2/V3 artifact authority route is
not part of the production pipeline.

Status: implementation contract. Bounded implementation and evidence paths now
exist, but this contract does not by itself grant runnable, verified, or
accepted authority.

## Profile

Scalar GEMM V1 computes row-major `f32` matrix multiplication on exact target
`gfx942:xnack-`:

```text
C[row, col] = sum(t = 0..k, A[row, t] * B[t, col])
```

The source signature is:

```rust,ignore
pub fn scalar_gemm_v1(
    a: &[f32],
    b: &[f32],
    c: DisjointSlice<f32>,
    m: u32,
    n: u32,
    k: u32,
)
```

One linear GPU invocation owns at most one output element. For invocation
index `p`, the kernel returns without a store unless `p < m * n`. An active
invocation computes `row = p / n` and `col = p % n`, then executes exactly `k`
iterations in increasing `t` order. The accumulator starts at positive
`0.0f32`; each iteration performs a separate `f32` multiply followed by a
separate `f32` add. Reassociation and contraction to FMA are forbidden.

The active guard implies `n != 0`, so integer division and remainder are never
evaluated with a zero divisor. A zero `m`, `n`, or output extent is a host
no-dispatch result with no device store. `k == 0` stores positive zero for each
active output.

## Host Admission

Host admission computes all extents with checked `u64` arithmetic, converts
them to the platform `usize` only after proving they fit, and requires:

- `a.len() == m * k`;
- `b.len() == k * n`;
- `c.len() == m * n`;
- `m * n` fits the authenticated launch domain;
- the complete byte ranges of `c` and `a` do not overlap;
- the complete byte ranges of `c` and `b` do not overlap.

The two read-only inputs may alias one another. Empty ranges do not overlap.
Pointer-plus-length byte ranges must not wrap. Length, dimension, allocation,
address-space, target, or profile substitution rejects before packing.

## Physical ABI

The COV6 explicit kernarg is 64 bytes, aligned to 8 bytes. Its fields are:

| Offset | Size | Field |
|---:|---:|---|
| 0 | 8 | `a` pointer |
| 8 | 8 | `a` length |
| 16 | 8 | `b` pointer |
| 24 | 8 | `b` length |
| 32 | 8 | `c` pointer |
| 40 | 8 | `c` length |
| 48 | 4 | `m` |
| 52 | 4 | `n` |
| 56 | 4 | `k` |
| 60 | 4 | zero tail padding |

The complete COV6 kernarg is 320 bytes after the canonical 256-byte implicit
suffix. The launch uses exactly 256 threads per workgroup and a one-dimensional
grid rounded up from `m * n` elements.

## Required Evidence

The run gate requires all of the following to agree on one identity-bound
source and artifact:

1. rustc collection authenticates one exact `KernelEntry`, signature, source
   body, direct calls, target, compiler semantics, and ABI layout.
2. Verified Kernel IR retains the active guard, cyclic `k` loop, three affine
   accesses, one injective output, and strict multiply-then-add order.
3. The standalone worker uses upstream LLVM and LLD library APIs to produce and
   inspect one COV6 HSACO. COMGR and command-line linking are forbidden.
4. The production Worker V3 application path must load those exact bytes and
   check a boundary-shape matrix against an independent CPU oracle with
   allocation canaries and read-only input comparisons. The former recovered
   Worker V2 HSA harness is deleted and grants no retained runtime evidence.

The verification gate separately requires Verus proofs of flattened-index
bounds, input initialization, output injectivity, loop invariants, and the
abstract dot-product recurrence. Those proofs do not establish IEEE-754
numerical refinement, compiler correctness, machine-code refinement, runtime
correctness, or hardware correctness. Numerical behavior remains a separate
machine and differential evidence obligation.

The evidence gate additionally requires protected, fresh, signed compiler and
MI300X results plus independent hostile review. Candidate-owned tests or an
unsigned artifact cannot promote this slice.

## Rejection Matrix

Tests must reject at least: wrong field type/order/name; wrong export, target,
XNACK, COV, workgroup, or grid; extra roots or calls; malformed or unbounded
loop structure; division without the active guard; length or extent overflow;
out-of-bounds affine input access; non-injective or alternate output index;
aliased output; accumulator reordering, reassociation, FMA contraction, or
wrong initial value; mutable input; artifact, symbol, ABI, proof, lineage, or
receipt substitution.
