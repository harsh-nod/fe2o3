# Inspect the source origin of an ordered assembly region

The normal exporter can now produce an opt-in, bounded source-origin report
beside raw diagnostic KIR V17. It identifies the **whole ordered region**, not
each instruction's macro-expansion ancestry. The
[2026-09-23 qualification](evidence/ordered-program-origin-20260923.md)
covers fresh ordinary source exports and independent inspection; it does not
close visualization milestone V3.

## Start with a registered Rust kernel

Use the supported gfx942 wave64 profile and the ordinary device crate:

```rust
#![no_std]
use fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn ordered_repeat_u32(
    mut output: DisjointSlice<u32>,
    a: u32,
    b: u32,
    c: u32,
) {
    let result = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
        init { mov(out, input0); }
        repeat(2) { add(out, out, input1); }
    };
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = result;
    }
}
```

The macro expands at compile time to one move and two adds. The numbers specify
declared register roles, not observations of physical register contents.
See [ordered authoring](ordered-program-authoring-v1.md) for the accepted
instruction subset and [repeat source acceptance](ordered-repeat-source-acceptance-v1.md)
for the complete source-fixture/dependency preparation.

## Export through the normal compiler

Build the exporter, its sibling extractor and backend DSO from the same current
checkout and pinned Rust toolchain. Build the ordinary inspection example too:

```sh
cargo build -p rustc-codegen-fe2o3 --lib \
  --bin fe2o3-export-sim --bin fe2o3-rustc-extract
cargo build -p fe2o3-kir-sim-cli \
  --example inspect_diagnostic_ordered_program_v17
```

Set the paths below to that matching build and your registered kernel crate.
The output parent must exist; both output files must be new.

```sh
ORIGIN_BIN_DIR=/absolute/path/to/matching-target/debug

"$ORIGIN_BIN_DIR/fe2o3-export-sim" --diagnostic-kir-v17 \
  --diagnostic-ordered-origin-v1 kernel.origin.json \
  --crate my_kernel_crate --output kernel.kir --target gfx942 \
  --target-dir target/fresh-origin-export -- \
  --manifest-path Cargo.toml --lib --offline
```

Replace `my_kernel_crate` with the actual Rust crate selector. The explicit
origin flag is valid only with this V17 diagnostic mode; V16 and simulation
bundle modes reject it. With the flag absent, the old export path and strict
inspection JSON remain unchanged. Do not set the internal origin environment
variable or reconstruct compiler crate bindings: the normal exporter/extractor
owns those bindings.

The two files are not an atomic filesystem transaction. If later publication
fails, earlier fresh output remains for inspection and the command fails.
A partial output pair is not successful evidence.

## Check the report against its exact KIR

Run the unchanged inspector with a valid request for the same kernel:

```sh
"$ORIGIN_BIN_DIR/examples/inspect_diagnostic_ordered_program_v17" \
  kernel.kir kernel.request.json
```

For the example above, this compact inspection request selects one complete
64-lane workgroup and a one-element output view with guard words:

```json
{
  "schema": "fe2o3-simulation-request-v1",
  "kernel": "ordered_repeat_u32",
  "grid": [64, 1, 1],
  "workgroup": [64, 1, 1],
  "arguments": [
    {"kind":"buffer_view","backing":1,"element":"u32","access":"read_write","alignment":4,"byte_offset":4,"elements":1},
    {"kind":"scalar","type":"u32","bits":"0x00000000"},
    {"kind":"scalar","type":"u32","bits":"0xffffffff"},
    {"kind":"scalar","type":"u32","bits":"0x00000000"}
  ],
  "shared_buffers": [
    {"id":1,"element":"u32","access":"read_write","alignment":4,"bytes":"0xa5a5a5a5a5a5a5a5a5a5a5a5","initialized":"0x0000"}
  ]
}
```

This inspector performs structural inspection and CPU preflight, not kernel
execution. Compare its canonical digest/length, target/wave, declared source IDs,
KIR coordinate and raw block, descriptor order and register roles with the report.
The canonical identity is domain-separated: it is **not** the ordinary SHA-256
of the raw file. Retain both identities separately.

The report also binds semantic MIR, retained source inventory/preflight, root
function/monomorphization and the original rustc MIR body/block. Its live compiler
join uses the existing block identity and retained semantic-to-KIR correspondence,
not guessed line numbers or a matching instruction list.

## Read the origin fields precisely

| Field | Meaning |
| --- | --- |
| `expansion` | Compiler-recorded expansion span and file identity. |
| `call_site` | Separate compiler-recorded source call-site span and file identity. |
| `expansion_chain_sha256`, `expansion_depth` | Actual retained expansion-chain digest and observed depth, not a reconstructed tree. |
| `declared_instructions` | Ordered descriptors, each associated with the whole region only. |
| `rustc_mir_block`, `semantic_block`, `kir_roster_coordinate` | Different coordinate spaces; they are not interchangeable indexes. |

The fresh one-, two- and fifteen-copy macro sources all reported depth **1**.
Repeat count is not macro depth. Zero depth is also a valid general observation.
File identities are compiler identities, not interchangeable with source-file
content hashes.

The report is limited to 16 KiB, at most 16 instructions, 256 macro parents and a
separate 1,048,576-unit source-reobservation work allowance. These are diagnostic
traversal/output bounds, not a whole-compiler allocation or RSS limit.

## Compare variants without inventing provenance

Export each edited source independently. Keep each report joined to its own
canonical and semantic identities even when coordinates or instruction bytes
coincide. Root/contract inventories may remain equal across edits; they are not
generic body digests. A fresh repeated export of the identical fifteen-copy
source reproduced the complete origin report in the qualification.

Fine-step origins and the full macro-frame list are explicitly unavailable.
This pre-ranked route does not compile native code, observe hardware, provide a
source-variable map, or prove physical register values/lifetimes. Compiler-policy,
edit-epoch, schedule and final-artifact bindings absent from this report remain
unavailable. Its JSON grants no proof, production-resume, artifact or launch
authority, and is not source or compiler-execution authentication.

The [LLVM](ordered-repeat-llvm-observation-v1.md) and
[native comparison](ordered-repeat-native-observation-v1.md) guides cover separate
captures. Do not attach this new report to historical native evidence by label,
coordinate, repeat count or equal bytes. An eventual viewer handoff must check
the actual shared identity bindings and preserve unavailable fields.
