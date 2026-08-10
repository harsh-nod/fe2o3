# S09 source-debug pilot V1

This pilot preserves source metadata for the authenticated General V3 `alpha`
kernel on exact target `gfx942:xnack-`. It is enabled only by Worker V2
configuration value `s09-alpha-gfx942-o0-v1`, which requires COV6, `O0`,
`strip-debug=false`, and per-stage LLVM verification.

The profile is deliberately closed, but it does not treat a Cargo/rustc symbol
as stable source identity. Manifest V2 separates `SemanticIdentityClaimV2`
from `BuildIdentityClaimV2`. The semantic claim binds the canonical source
path, 3,231-byte length, source SHA-256, logical crate/module/name/export,
General Scalar/Slice rustc-layout V3 profile, portable MIR, ABI, launch shape,
and the exact gfx942/COV6/O0 debug policy. The build claim records Cargo
metadata, crate and kernel bindings, observed DefPath and symbol, the final
prepared rustc command and rustc executable, `cargo-fe2o3`, the declared Cargo
executable, the Cargo launcher executable, parent PID and process start time,
backend, Worker V2, and LLVM identities. Both records are inert claims:
decoding establishes canonical syntax and digest linkage but grants no
authority. Evidence policy separately authenticates and binds their values.

Cargo launcher provenance is rooted in an inherited pinned executable
descriptor. The wrapper verifies that descriptor against its trusted digest
and requires `/proc/<parent-pid>/exe` to name the same open executable object.
The PID and process start time are continuity observations only; they detect a
parent change while `/proc` is inspected and do not authenticate an
executable by themselves. The prepared-command digest covers the final
program, resolved pinned rustc identity, working directory, complete argument
vector, and complete S09 child environment. The wrapper transfers that digest
to codegen through an exact sealed 32-byte prepared-command capability at a
fixed inherited descriptor. Codegen requires the exact immutable seal set and
rejects a missing, replaced, writable, resized, zero, or trailing capability.

The observed DefPath and symbol are opaque, canonical build observations.
Their exact values may change when Cargo `-C metadata`, the dependency graph,
toolchain, profile, or checkout context changes. The harness does not derive
either value from a binding ID and does not admit a build by matching a fixed
DefPath or symbol. Such changes produce a new `BuildIdentityClaimV2`; they do
not alter `SemanticIdentityClaimV2` when source, portable MIR semantics, ABI,
launch shape, and target policy are unchanged.

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
final continuation. ROCgdb runs with `--return-child-result`. Only after the
hardware test exits normally with child status zero does a literal conditional
ROCgdb command emit one `FE2O3_S09_HARNESS_RESULT_V1` marker carrying the HSACO
digest, a runner-generated 256-bit nonce, and `result=passed`. The checker does
not depend on Cargo status-line adjacency: it accepts bounded debugger
thread-exit interleaving, then requires normal inferior exit, the conditional
runner marker, debugger hardware-pass marker, and zero ROCgdb exit status in
that order. Removing, moving, or forging the marker fails closed, as do
reordered stops, a substitute host `alpha`, digest/build-ID mismatch, or an
unavailable observation.

Normalized DWARF, transcript, and facts files use ordered field schemas and
strict value grammars. They reject file URIs, URIs carrying paths,
percent-encoded paths, absolute POSIX paths, Windows drive/UNC/device paths,
and dot components regardless of surrounding delimiters. The only admitted
Rust source path is the exact relative S09 fixture path. Raw logs are never
authoritative artifacts.

The production entry point is `check-production`. It has no manifest, digest,
policy, or trust-path arguments. It reads only the compiled fixed path
`/etc/fe2o3/s09-trust-v2.tsv` using one `O_NOFOLLOW` descriptor. The
policy must be a root-owned, single-link regular file with no write bits and
the Linux filesystem immutable flag. Missing or unsupported installation
fails closed. The policy binds the canonical installed manifest path and
digest plus every `SemanticIdentityClaimV2` and `BuildIdentityClaimV2` field.

The installed manifest uses ordered schema
`fe2o3-s09-protected-manifest-v2` and has exactly 54 fields. The count is
derived from the codec field lists: three envelope fields, 41 identity fields,
and ten evidence fields. The identity fields are namespaced, exact copies of:

- the section name and SHA-256 of the canonical 18-field
  `SemanticIdentityClaimV2` record, named `semantic_claim_sha256`;
- every semantic record field in codec order;
- the SHA-256 of the canonical 20-field `BuildIdentityClaimV2` record, named
  `build_claim_sha256`; and
- every build record field in codec order.

The final ten fields bind the source commit/tree, exact HSACO, host executable
and build ID, archive manifest, artifact facts, hardware facts, normalized
DWARF, and normalized ROCgdb transcript. The checker reads the HSACO itself,
requires exactly one `.fe2o3.s09.identity.v2` `SHT_PROGBITS` section, decodes
both bounded records, verifies the handoff's exact semantic and build claim
digests plus the build-to-semantic digest edge, and compares all 41 derived
identity fields against the protected manifest. Identity values are never
accepted from environment variables or command-line field values.

The compiler codec additionally delegates HSACO inspection to the physical
kernel-descriptor binder before returning the inert claims. Metadata, entry
symbols, descriptor symbols, descriptor bytes, and kernel cardinality must
form one exact closed physical set, and the semantic export must resolve to
one member of that set. This structural binding does not itself authenticate
the claims or replace production evidence policy.

Every digest is lowercase, nonzero SHA-256. Serialization is one UTF-8,
LF-terminated `field<TAB>value` line per field in fixed order. Missing,
duplicate, reordered, empty, truncated, oversized, trailing, unknown, zero
digest, or noncanonical fields fail closed. ELF size, section count, string
table, handoff, record, field-name, and field-value sizes are explicitly
bounded. The installed immutable policy repeats and binds every manifest
field, while its own manifest digest binds the complete serialized byte
sequence.

Production accepts only `trust_domain=production-v2`. `check-fixture` is a
separate, explicitly non-authoritative test command. It accepts only
`trust_domain=test-fixture-v2`, cannot read production trust, and never emits a
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
test in a fresh isolated target directory, runs native ROCgdb, emits a
deterministic `s09-evidence-manifest-v2.tsv`, and validates its local evidence
bundle. The lane derives both identity records exclusively from the emitted
HSACO. Its `trust_domain=local-capability-v2` prevents promotion: local
selection of the checkout and host executable is not an evidence-grade
provenance boundary. A future protected controller must select immutable
inputs and the GPU runner, then install a separately measured `production-v2`
manifest and policy. Generic CI continues to run field-by-field mutation,
missing, zero-digest, duplicate, reorder, truncation, oversize, trailing-data,
unknown-field, deterministic-serialization, and lane-guard tests; neither
those tests nor a local pilot archive can satisfy production debug evidence.

Pilot evidence classes are:

- `unit`: metadata injection, source-shape, and checker mutation tests;
- `ui`: closed Worker V2 configuration admission and rejection diagnostics;
- `compile`: genuine rustc/LLVM/LLD COV6 emission plus DWARF verification; and
- `debug`: normalized `llvm-dwarfdump` and native ROCgdb archive inspection.

This pilot is not production parity evidence. S09 remains `Missing` until a
production debug result is signed and accepted by the parity gate.
