# fe2o3

`fe2o3` is an experimental single-source Rust GPU stack for AMD GPUs.

The intended end state is:

```text
Rust host + #[kernel] device code
        |
        v
rustc frontend and MIR
        |
        +--> native host binary
        |
        +--> fe2o3 device backend -> AMDGPU LLVM IR -> HSACO
                                                |
                                                v
                                     HIP module load/launch
```

This initial implementation establishes the crate layout and runtime/compiler
interfaces needed to reach that target:

- `fe2o3-device`: no-std device API and `#[kernel]` re-export.
- `fe2o3-macros`: kernel marker macro and reserved symbol naming.
- `fe2o3-core`: HIP-backed host runtime, buffers, streams, modules, launches.
- `fe2o3-host`: user-facing launch macro.
- `rustc-codegen-fe2o3`: backend support code and HSACO toolchain hooks.
- `cargo-fe2o3`: cargo subcommand and environment diagnostics.
- `dialect-amdgcn`: AMDGPU intrinsic naming seam for the future Pliron lowering.
- `dialect-mir`: Rust MIR dialect naming seam for the future Pliron lowering.

## Current Status

The HIP runtime layer and public API are working for the current elementwise
examples on AMD hardware. `cargo-fe2o3 build` builds and loads
`librustc_codegen_fe2o3.so`, delegates host codegen through
`rustc_codegen_llvm`, detects `#[kernel]` functions in rustc codegen units, and
dumps the currently collected device-reachable MIR functions. When
`FE2O3_DUMP_MIR=1` is set, it also prints the first Pliron-facing MIR import
scaffold for the collected device functions using local `mir.*` dialect names
and builds a flat typed `mir.*` operation-record stream for the future Pliron
builder, including typed locals, statement destination and operand labels, and
the first operation-specific lowering records such as `mir.load`, `mir.store`,
arithmetic ops, comparisons, and casts. The same dump also builds a
record-driven lowering-plan summary from that flat record stream.

The backend also has the first AMDGPU artifact path: for the current `f32`/`f64`
elementwise kernel shapes it validates the Rust kernel ABI from monomorphized
MIR argument locals, recognizes a small expression tree, emits a minimal AMDGPU
LLVM IR kernel, and compiles it through ROCm clang plus `ld.lld` into
`target/fe2o3/*.hsaco`. Supported expression leaves are read-only slice
elements, plain scalar float arguments, float literals, and the mutable output
slice when doing an in-place update. The same shape is supported for `f64`.
Outputs can be `DisjointSlice<T>` or indexed `&mut [T]`; expression nodes
include `+`, `-`, `*`, `/`, and unary negation. Leaf-only copies such as
`out[i] = x[i]` and literal-root fills are also supported.
`DisjointSlice::get_mut` outputs can read the current element before writing it,
and `DisjointSlice::get_mut_at` supports raw `usize` output indexes.
`ThreadIndex::offset` supports simple constant-offset slice reads such as
`x[idx.offset(1)]`; `ThreadIndex::offset_signed` supports signed constant
offsets such as `x[idx.offset_signed(-1)]`.
`ThreadIndex::stride` supports constant-stride reads such as `x[idx.stride(2)]`.
`ThreadIndex::stride_offset` supports affine reads such as
`x[idx.stride_offset(2, 1)]`.
Raw `usize` index arithmetic derived from `idx.get()` is also recognized for
constant add, subtract, and multiply patterns such as `idx.get() * 2 + 1`, plus
affine combinations of two tracked index expressions such as
`idx.get() + idx.get() + 1`, and constant-minus-index forms such as
`1023 - idx.get()`.
General MIR/Pliron lowering is still the next compiler milestone.

On a `gfx1201` AMD Radeon AI PRO R9700 with TheRock ROCm
`7.13.0a20260509`, `cargo-fe2o3 run -p fe2o3-vecadd`,
`cargo-fe2o3 run -p fe2o3-scale`, `cargo-fe2o3 run -p fe2o3-saxpy`, and
`cargo-fe2o3 run -p fe2o3-axpy-inplace` generate HSACO artifacts, load them
through HIP, launch the kernels, and validate the results. `fe2o3-negate` covers
unary negation. `fe2o3-normalize` covers literal constants plus subtraction and
division. `fe2o3-copy` covers leaf-only stores.
`fe2o3-downsample` covers constant-stride input loads.
`fe2o3-gather-odd` covers stride-plus-offset input loads.
`fe2o3-raw-add-index` covers affine reads formed by adding two raw index
expressions.
`fe2o3-raw-const-minus` covers constant-minus-index reads with a negative
stride.
`fe2o3-raw-parenthesized-sub` covers parenthesized index subtraction that
collapses to a constant read index.
`fe2o3-raw-disjoint-inplace-shift` covers raw `usize` arithmetic for a
`DisjointSlice<f32>` output read-before-write store.
`fe2o3-raw-disjoint-shift` covers raw `usize` arithmetic for a
`DisjointSlice<f32>` output store.
`fe2o3-raw-gather` covers raw affine `usize` index arithmetic.
`fe2o3-raw-neighbors` covers raw `usize` add/sub neighbor reads.
`fe2o3-raw-output-shift` covers raw `usize` arithmetic for an indexed
`&mut [f32]` output store.
`fe2o3-add-inplace` covers read-before-write through `DisjointSlice::get_mut`.
`fe2o3-fill` covers literal-root stores with no input loads.
`fe2o3-shift` covers constant-offset input loads.
`fe2o3-previous` covers negative constant-offset input loads.
`fe2o3-stencil` covers multiple derived loads from one input slice.
`fe2o3-vecadd-f64` covers double-precision elementwise lowering.
`cargo-fe2o3 run -p fe2o3-pipeline` emits and launches two kernels from one Rust
crate.

See [docs/implementation-plan.md](docs/implementation-plan.md) for the full
compiler/runtime plan.

Run diagnostics:

```bash
cargo run -p cargo-fe2o3 -- doctor
```

If `FE2O3_TARGET` is not set, `cargo-fe2o3` tries to infer the target from
`rocminfo` and falls back to `gfx1100`.

For explicit package builds such as `-p fe2o3-saxpy`, `cargo-fe2o3` cleans that
package before invoking Cargo so sidecar HSACO files are regenerated even if the
host crate was already up to date.

Check the workspace:

```bash
cargo check --workspace
```

Smoke-test the current backend entry point:

```bash
cargo run -p cargo-fe2o3 -- build -p fe2o3-vecadd
cargo run -p cargo-fe2o3 -- build -p fe2o3-add-inplace
cargo run -p cargo-fe2o3 -- build -p fe2o3-copy
cargo run -p cargo-fe2o3 -- build -p fe2o3-downsample
cargo run -p cargo-fe2o3 -- build -p fe2o3-fill
cargo run -p cargo-fe2o3 -- build -p fe2o3-gather-odd
cargo run -p cargo-fe2o3 -- build -p fe2o3-scale
cargo run -p cargo-fe2o3 -- build -p fe2o3-shift
cargo run -p cargo-fe2o3 -- build -p fe2o3-previous
cargo run -p cargo-fe2o3 -- build -p fe2o3-stencil
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-add-index
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-const-minus
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-parenthesized-sub
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-disjoint-inplace-shift
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-disjoint-shift
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-gather
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-neighbors
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-output-shift
cargo run -p cargo-fe2o3 -- build -p fe2o3-saxpy
cargo run -p cargo-fe2o3 -- build -p fe2o3-axpy-inplace
cargo run -p cargo-fe2o3 -- build -p fe2o3-negate
cargo run -p cargo-fe2o3 -- build -p fe2o3-normalize
cargo run -p cargo-fe2o3 -- build -p fe2o3-pipeline
cargo run -p cargo-fe2o3 -- build -p fe2o3-vecadd-f64
```

On a machine with a working AMD GPU and ROCm driver stack:

```bash
cargo run -p cargo-fe2o3 -- smoke
```

The smoke command runs the supported backend examples in sequence. To run one
package at a time:

```bash
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd
cargo run -p cargo-fe2o3 -- run -p fe2o3-add-inplace
cargo run -p cargo-fe2o3 -- run -p fe2o3-copy
cargo run -p cargo-fe2o3 -- run -p fe2o3-downsample
cargo run -p cargo-fe2o3 -- run -p fe2o3-fill
cargo run -p cargo-fe2o3 -- run -p fe2o3-gather-odd
cargo run -p cargo-fe2o3 -- run -p fe2o3-scale
cargo run -p cargo-fe2o3 -- run -p fe2o3-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-previous
cargo run -p cargo-fe2o3 -- run -p fe2o3-stencil
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-add-index
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-const-minus
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-parenthesized-sub
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-disjoint-inplace-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-disjoint-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-gather
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-neighbors
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-output-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-saxpy
cargo run -p cargo-fe2o3 -- run -p fe2o3-axpy-inplace
cargo run -p cargo-fe2o3 -- run -p fe2o3-negate
cargo run -p cargo-fe2o3 -- run -p fe2o3-normalize
cargo run -p cargo-fe2o3 -- run -p fe2o3-pipeline
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd-f64
```
