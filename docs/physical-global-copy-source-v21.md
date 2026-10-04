# Authored readonly global-copy source diagnostics (V21)

This bounded route authors the complete gfx942 entry body in Rust, including
kernarg reads, a global load, explicit waits, output masking, store, EXEC restore
and termination. It does not add a compiler-owned setup or tail.

The current endpoint is **pre-ranked and diagnostic-only**. It retains genuine
source ownership internally through MIR38, actual KIR21 SSA construction,
canonical admission and LLVM emission. Exported KIR, LLVM and native-observation
text are inert files: they cannot recreate source custody, satisfy host memory
conditions or authorize a native artifact or GPU launch. Qualification results
must be recorded separately; this page is not a successful-run receipt.

## Source shape

See the complete runnable
[fixture](../crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/physical_global_copy_v21.rs).
The two logical parameters are `input: &[u32]` and
`output: DisjointSlice<u32>`, with actual shared-borrow and exclusive-owner
source identities. Their four native components are input pointer, input length,
output pointer and output length at offsets 0, 8, 16 and 24. No ghost allocation
or extra logical parameter represents the compiler kernarg segment.

The profile is gfx942:xnack-, Wave64, COV6, required/max workgroup [64,1,1],
explicit max_grid [2,1,1], and one authored block. The complete source includes:

```rust
amdgpu_physical_global_copy! {
    gfx942_xnack_off_wave64;
    input=input; output=output;
    label(0);
    // Four exact kernarg pair loads and an explicit LGKM wait precede
    // authored global-index and input-address construction.
    // ... see the complete fixture; these comments are not implicit code ...
    global_load_dword(v(8), v_pair(6));
    s_waitcnt_vmcnt0();
    // Authored output-address construction and output bounds mask follow.
    // ... see the complete fixture ...
    global_store_dword(v_pair(10), v(8));
    s_waitcnt_vmcnt0();
    s_mov_b64_exec(s_pair(18));
    s_endpgm0();
}
```

This excerpt is intentionally incomplete; compile the linked full source.
Registers name physical units, while the importer creates actual SSA values and
checks each use against the current register definition. The global read is
full-EXEC, before the output guard. An empty/short output does not excuse an
out-of-bounds or uninitialized input. The load result must be completed by the
immediately following VM wait and the store must consume that exact result.
A same-sized constant or another register is not an equivalent source value.

## Public source command

Build using the repository's pinned nightly 2026-04-03 and normal offline
dependencies. Set `RUSTC` to that compiler's absolute path and retain the actual
build outputs; the script does not auto-install a compiler.

```sh
cargo build --locked --offline -p rustc-codegen-fe2o3 --lib --bin fe2o3-rustc-extract
node scripts/physical-global-copy-source-v21.mjs one /absolute/new-copy-output
node scripts/physical-global-copy-source-v21.mjs registers /absolute/new-register-output
```

If the build uses a nondefault target directory, set
`FE2O3_PHYSICAL_GLOBAL_COPY_BIN_DIR_V21` to its debug binary directory.
Each output directory must be new. The command invokes the ordinary Cargo
workspace wrapper with the explicit
`FE2O3_EXTRACT_DIAGNOSTIC_PHYSICAL_GLOBAL_COPY_DIRECTORY_V21` selector; it
rejects mixed output modes. Dependencies remain ordinary compiler passthrough.

A successful source export contains `canonical-v21.bin`, unchanged
`canonical.ll`, and `native-observation-input-v21.txt` under `diagnostic/`,
plus source/binary pins and process observations. The script does not invoke a
native worker, simulator, artifact finalizer, runtime or debugger. Its process,
file and observed-disk bounds are not a whole-machine RSS/allocator guarantee.

Use separate fresh directories for the precise negative controls:
`wrong-launch`, `missing-lgkm`, `missing-vm`, `wrong-carry`,
`wrong-store`, `wrong-offset`, and `reserved-register`. Each command requires
its named refusal stage; a generic failure or signal does not count as success.
Refused source must not create a diagnostic output directory.

## Reproducible qualification and limits

The ignored backend ladder
`actual_physical_global_copy_source_ladder` accepts a fresh absolute
`FE2O3_TEST_PHYSICAL_GLOBAL_COPY_OUTPUT_V21`. It runs 18 isolated actual-rustc
sessions for nine source variants and two paths (owned observation/public
diagnostic driver). Intended checks are byte-identical public output for both
positive variants, 128 CPU positive cases, six CPU input-bounds/initialization/
same-backing refusals, and 14 precise source refusals. These are test
expectations, not claims that the ladder has passed.

CPU observation uses the existing simulator, the same source-retaining
verification ledger for admission, and the engine's separately bounded
execution domain. It compares copied values with an independent input pattern,
preserves input and two canary words on both sides of output, and checks the
full-EXEC read even when no store lane is active. Ragged output lengths are CPU
mask controls, not production formal-memory certificates.

Ranked/formal-memory/descriptor continuation is not provided by this endpoint.
Future ordinary production admission must retain real read/write obligations
for both allocations, conservative launch-envelope bounds (512 bytes each for
128 invocations), input initialization/read permission, output write permission,
input/output nonaliasing, and compiler-kernarg ABI lifetime/readability/
immutability/disjointness conditions. None are discharged by serialized
diagnostics, logical argument IDs, or the Rust types alone at a detached host
binding. The existing generic formal analyzer remains fail-closed for these
operations.

This slice does not complete the broader memory/synchronization milestone.
LDS, barriers, arbitrary outstanding operations, alternate waits, loops,
physical debugger capture, source-variable maps, protected finalization and
GPU execution remain outside this diagnostic profile.
