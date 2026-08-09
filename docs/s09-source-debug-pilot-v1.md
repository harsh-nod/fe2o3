# S09 source-debug pilot V1

This pilot preserves source metadata for the authenticated General V3 `alpha`
kernel on exact target `gfx942:xnack-`. It is enabled only by Worker V2
configuration value `s09-alpha-gfx942-o0-v1`, which requires COV6, `O0`,
`strip-debug=false`, and per-stage LLVM verification.

The profile is deliberately closed. The compiler binds the local crate name,
the macro-generated alpha DefPath, the SHA-256 of the complete checked-in
source file, and the canonical remapped path
`crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs`.
Absolute paths and paths containing `.` or `..` components are rejected and
the checkout root is not emitted into DWARF. The compiler also requires the
function at line 68, index statement at line 69, local `i` at line 70, and the
exact `f32`, read-only slice, and `DisjointSlice<f32>` argument profile. It
emits DWARF for:

- function `alpha` and its source file/line;
- scalar argument `scale`;
- the physical slice components `input_data`, `input_len`, `output_data`, and
  `output_len`; and
- scalar local `i` after the trusted global-index operation.

Rust slice and `DisjointSlice` values are represented by their physical
pointer/length components. Aggregate reconstruction, structs, tuples, arrays,
optimized debugging, arbitrary source profiles, and user-provided debugger
commands are unsupported. Requesting the profile with optimization or debug
stripping fails before compiler execution. A source, ABI, SSA-shape, target, or
pre-existing debug-metadata mismatch fails before Worker V2 publication.

Source spelling is not sufficient admission. The imported rustc MIR must have
the exact bounded alpha shape: eight blocks, fourteen locals, three arguments,
the authenticated thread-index/get-mut/index-get calls, one output-option
switch, one input-bounds assertion, one indexed input load, one `f32` multiply,
and one direct guarded output store. Semantic-body or control-flow drift fails
before debug metadata is injected.

The O0 profile emits three fixed compiler-internal `s_nop 0` debug witnesses.
The line-68 staging witness assigns kernarg setup to the function line, the
line-69 witness keeps the five physical ABI values live for argument
inspection, and the line-70 witness keeps `i` live for local inspection. These
witnesses are not source inline assembly and are never accepted from user
input. They are an intentional code-generation cost of this exact debug
profile.

The compile test emits one COV6 HSACO and requires `llvm-dwarfdump --verify` to
accept its linked DWARF. The fixed ROCgdb runner and transcript checker are a
separate debug evidence boundary. The runner accepts only an absolute HSACO,
the fixed hardware-test executable, and a fresh archive directory. It invokes
native `/opt/rocm/bin/rocgdb-py_3.12` with literal batch commands and accepts no
debugger command, init file, or command environment input.

Before ROCgdb runs, the runner derives the exact HSACO SHA-256, hardware-test
SHA-256, and hardware ELF GNU build ID. It derives `gfx942:xnack-` from AMDGPU
metadata and `O0` from the exact inspected DWARF producer/configuration. Those
facts are bound into both the normalized transcript and archive manifest. The
hardware test independently reads the exact HSACO bytes under the same digest.
The transcript must show the pending `alpha` breakpoint resolve after kernel
load, an AMDGPU wave and lane stopped in canonical source line 68, a loaded
in-memory AMDGPU code object, all five physical arguments at line 69, `i` at
line 70, and the exact hardware test completing with a normal inferior exit.
A substitute host `alpha`, digest/build-ID mismatch, missing hardware pass, or
unavailable observation fails even when ROCgdb exits zero.

The available real lane is explicit and GPU-gated:

```text
FE2O3_ALLOW_S09_DEBUG=1 \
FE2O3_LLVM_LINK_WORKER=/absolute/fe2o3-llvm-link-worker \
FE2O3_LLVM_LINK_WORKER_BUILD_ID=<measured-worker-id> \
FE2O3_LLVM_BUILD_ID=<measured-llvm-id> \
FE2O3_S09_EVIDENCE_DIR=/absolute/new-evidence-directory \
  scripts/ci-local.sh s09-debug-hardware
```

This lane performs the genuine direct LLVM/LLD compile, builds the hardware
test in a fresh isolated target directory, runs native ROCgdb, and archives the
bound artifact, executable, DWARF, transcript, and status facts. The manual
`s09-debug.yml` workflow exposes the same lane on a self-hosted ROCm AMD-GPU
runner. Generic CI continues to run all synthetic mutation and lane-guard
tests, but those tests cannot satisfy real debug evidence.

Pilot evidence classes are:

- `unit`: metadata injection, source-shape, and checker mutation tests;
- `ui`: closed Worker V2 configuration admission and rejection diagnostics;
- `compile`: genuine rustc/LLVM/LLD COV6 emission plus DWARF verification; and
- `debug`: normalized `llvm-dwarfdump` and native ROCgdb archive inspection.

This pilot is not production parity evidence. S09 remains `Missing` until a
production debug result is signed and accepted by the parity gate.
