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

Before ROCgdb runs, the local runner measures the HSACO SHA-256, hardware-test
SHA-256, and hardware ELF GNU build ID. It derives `gfx942:xnack-` from AMDGPU
metadata and `O0` from the inspected DWARF producer/configuration. These are
capability measurements, not authenticated provenance: the same local process
selected the executable and measured it. The raw ROCgdb log is hashed and then
removed. Only the path-clean normalized transcript is retained.

The normalized transcript is segmented around every debugger continuation.
The checker requires an exact BP2 line-69 hit followed by an AMDGPU-wave frame
and all physical argument observations before BP3 is armed. It then requires
an exact BP3 line-70 hit, another AMDGPU-wave frame, and local `i` before the
final continuation. After all hardware checks and unload complete, the bound
Rust harness emits one exact `FE2O3_S09_HARNESS_RESULT_V1` marker carrying the
HSACO digest, a runner-generated 256-bit nonce, and `result=passed`. The checker
does not depend on Cargo status-line adjacency: it accepts bounded debugger
thread-exit interleaving, then requires the harness marker, normal inferior
exit, debugger hardware-pass marker, and zero ROCgdb exit status in that order.
Removing or forging the harness marker fails closed, as do reordered stops, a
substitute host `alpha`, digest/build-ID mismatch, or unavailable observation.

Normalized DWARF, transcript, and facts files use ordered field schemas and
strict value grammars. They reject file URIs, URIs carrying paths,
percent-encoded paths, absolute POSIX paths, Windows drive/UNC/device paths,
and dot components regardless of surrounding delimiters. The only admitted
Rust source path is the exact relative S09 fixture path. Raw logs are never
authoritative artifacts.

The production entry point is `check-production`. It has no manifest, digest,
policy, or trust-path arguments. It reads only the compiled fixed path
`/etc/fe2o3/s09-trust-v1.tsv` using one `O_NOFOLLOW` descriptor. The
policy must be a root-owned, single-link regular file with no write bits and
the Linux filesystem immutable flag. Missing or unsupported installation
fails closed. The policy binds the canonical installed manifest path and
digest plus the exact source commit/tree, toolchain, checker, harness, HSACO,
host executable, and host build-ID identities.

The installed manifest uses ordered schema
`fe2o3-s09-protected-manifest-v1` and binds:

- production trust domain, profile, claim, and protected execution closure;
- exact source commit, source tree, source path, and source SHA-256;
- exact rustc, LLVM link worker, LLD, dwarfdump, readobj, and ROCgdb digests;
- exact checker and hardware-harness source digests;
- exact HSACO and host executable digests plus host build ID; and
- exact normalized artifact-facts, hardware-facts, DWARF, and ROCgdb digests.

Production accepts only `trust_domain=production-v1` and
`execution_closure=protected-controller-v1`. `check-fixture` is a separate,
explicitly non-authoritative test command. It accepts only
`trust_domain=test-fixture-v1`, cannot read production trust, and never emits a
production-success message. A caller-supplied manifest or digest cannot reach
the production command. A future protected controller and administrator must
construct and install the production policy and manifest after selecting
immutable inputs.

The available real lane is an explicit, GPU-gated local capability pilot:

```text
FE2O3_ALLOW_S09_DEBUG=1 \
FE2O3_LLVM_LINK_WORKER=/absolute/fe2o3-llvm-link-worker \
FE2O3_LLVM_LINK_WORKER_BUILD_ID=<measured-worker-id> \
FE2O3_LLVM_BUILD_ID=<measured-llvm-id> \
FE2O3_S09_EVIDENCE_DIR=/absolute/new-evidence-directory \
  scripts/ci-local.sh s09-debug-hardware
```

This lane performs the genuine direct LLVM/LLD compile, builds the hardware
test in a fresh isolated target directory, and runs native ROCgdb. Its output
is capability-only: local selection of the checkout and host executable is not
an evidence-grade provenance boundary. The candidate-controlled self-hosted
workflow was removed. A future protected controller must select an immutable
source revision, toolchain, harness, and GPU runner, then supply their exact
digests in the protected S09 manifest. Generic CI continues to run synthetic
mutation and lane-guard tests; neither those tests nor a local pilot archive
can satisfy production debug evidence.

Pilot evidence classes are:

- `unit`: metadata injection, source-shape, and checker mutation tests;
- `ui`: closed Worker V2 configuration admission and rejection diagnostics;
- `compile`: genuine rustc/LLVM/LLD COV6 emission plus DWARF verification; and
- `debug`: normalized `llvm-dwarfdump` and native ROCgdb archive inspection.

This pilot is not production parity evidence. S09 remains `Missing` until a
production debug result is signed and accepted by the parity gate.
