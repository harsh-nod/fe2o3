# S09 source-debug pilot V1

This pilot preserves source metadata for the closed General V3 `alpha` kernel
profile on exact target `gfx942:xnack-`. It is enabled only by Worker V2
configuration value `s09-alpha-gfx942-o0-v1`, which requires COV6, `O0`,
`strip-debug=false`, and per-stage LLVM verification.

The profile is deliberately closed, but it does not treat a Cargo/rustc symbol
as stable source identity. Manifest V2 separates `SemanticIdentityClaimV2`
from `BuildIdentityClaimV2`. The semantic claim binds the canonical source
path, 3,359-byte length, source SHA-256
`73c1ff5e2f29d245c8071bdb6c1a38af1c9ee1573b78d47a987633483b37e084`,
logical crate/module/name/export, General Scalar/Slice rustc-layout V3 profile,
portable MIR, ABI, launch shape, and the exact gfx942/COV6/O0 debug policy. The
build claim records Cargo
metadata, crate and kernel bindings, observed DefPath and symbol, the final
prepared rustc command and rustc executable, `cargo-fe2o3`, the declared Cargo
executable, the brokered pinned Cargo image digest, observed parent PID and
process start time, backend, Worker V2, and LLVM identities. Both records are
inert claims:
decoding establishes canonical syntax and digest linkage but grants no
authority. Evidence policy separately authenticates and binds their values.

The capability broker transfers an open pinned Cargo image to the wrapper.
The wrapper measures `pinned_cargo_image_sha256` from that object and then
drops its local pin. This is a brokered build observation; it does not prove
which process launched the wrapper. `observed_parent_pid` and
`observed_parent_start_time_ticks` report the actual parent observed while the
wrapper reads `/proc/<pid>/stat`. Re-reading the start time and parent PID
detects a process change during that observation, but neither value
authenticates Cargo or binds the parent to the pinned image.

The wrapper pins the final rustc executable and working-directory objects. A
descriptor-based `fchdir` selects that exact directory immediately before
exec. Relative to the same directory descriptor, the wrapper traverses the
canonical protected source path without following symlinks and measures the
exact source length and SHA-256. The resulting source-tree identity binds the
cwd object, relative source path, source length, and source digest. The inert
process-consistency digest covers the executable object and bytes, raw argv
including argv0, cwd object, protected source-tree identity, and the complete
sorted child environment.

For S09, the wrapper rejects credential-like inherited variables, admits only
the required inherited `CARGO_MANIFEST_DIR`, fixed
`FE2O3_CODEGEN_PIPELINE=kernel-ir-worker-v2`, and fixed
`FE2O3_TARGET=gfx942:xnack-` inputs, applies the closed managed environment,
then calls `env_clear()` and installs that exact environment. The parent puts
the prepared consistency digest in an exact sealed 32-byte expectation at the
fixed descriptor. After exec, the running compiler independently remeasures
its executable, argv, cwd, protected source tree, and complete environment,
reconstructs the digest, and compares it with the sealed parent expectation.
Missing, replaced, writable, resized, zero, trailing, or inconsistent data
fails closed. This comparison detects parent/child process-input drift; it is
not an authentication or loader-history claim.

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
the compiler-recognized thread-index/get-mut/index-get calls, one output-option
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

The compile test emits one alpha-only COV6 HSACO, rejects any physically bound
`zeta` entry, and requires `llvm-dwarfdump --verify` to accept its linked
DWARF. The fixed ROCgdb runner and transcript checker are a separate debug
evidence boundary. The S09 Rust controller is separate from the ordinary
alpha/zeta controller and accepts only the exact alpha-only artifact facts,
their SHA-256, and the matching HSACO SHA-256. The fixed facts are
`gfx942:xnack-`, O0, and one `alpha` entry bound to one `alpha.kd` descriptor.
The controller then requires the 40-byte explicit alpha layout plus the
256-byte COV6 implicit suffix, executes lengths 1, 255, 256, 257, and 1023,
and checks the CPU oracle, immutable input, and both output canaries. Extra
kernels, a changed descriptor symbol, either digest substitution, target
drift, or a changed runtime kernarg shape fail closed. The ordinary alpha/zeta
hardware tests remain unchanged and continue to cover the non-S09 two-kernel
contract.

Before ROCgdb runs, the local runner measures the HSACO SHA-256, hardware-test
SHA-256, and hardware ELF GNU build ID. It derives `gfx942:xnack-` from AMDGPU
metadata and `O0` from the inspected DWARF producer/configuration. These are
capability measurements, not authenticated provenance: the same local process
selected the executable and measured it. The lane first requires a clean exact
Git HEAD and seals a canonical inventory of every tracked regular blob. Each
entry binds the Git mode, raw path, Git blob object, content SHA-256, and Linux
device, inode, mode, link count, size, mtime, and ctime. A parent supervisor
holds that sealed inventory across compilation, debugging, and finalization,
then recaptures and byte-compares it. Same-size mutate/restore, mode restore,
and path-swap/restore operations therefore fail through content or inode
metadata drift even when the final Git status is clean.

Source-state capture opens the canonical `/usr/bin/git` object once and invokes
that held object through `/proc/self/fd/<n>` for every Git query. Each call
revalidates the fixed pathname, descriptor identity, executable content digest,
and proc-fd object. The child environment is constructed from scratch, disables
system and global Git configuration, and admits no inherited `GIT_*` values.
Tracked symlinks, gitlinks, and any Git mode other than `100644` or `100755`
fail closed instead of being omitted from the inventory.

The executed host bytes are copied atomically from the sealed memfd into the
private evidence directory as a single-link mode-0400 artifact. Downstream
inspection consumes that retained image. The final checker also snapshots and
parses the complete debug archive manifest, requires `result=passed` and every
tool status to be zero, and cross-binds the HSACO, retained host, checker,
facts, normalized DWARF, normalized ROCgdb, tool paths, and run nonce. The raw
ROCgdb log is hashed and then removed. Only the path-clean normalized
transcript is retained.

The public source-state supervisor owns a fresh session and process group for
the evidence lane. It catches `HUP`, `INT`, and `TERM`, forwards cancellation,
uses bounded `TERM`/`KILL` escalation before reaping the pinned group leader,
and repeats the sealed tracked-source comparison before returning. The nested
snapshot-supervised ROCgdb command has its own pinned process group. Its
`EXIT`, `HUP`, `INT`, and `TERM` cleanup unlinks the raw transcript first,
closes the parent-owned descriptor, and sends bounded group-wide `TERM`/`KILL`
before reaping its leader. For these caught signals, an interrupted public run
cannot defer raw-path cleanup until a foreground debugger returns.

These local process-group controls cover processes that remain in the pinned
groups. They do not prove containment of a descendant that creates another
session or process group, and they do not claim that every zombie has
disappeared. The raw pathname is checked for removal; complete descendant
lifetime ownership remains a `production-v2` obligation.

Uncatchable `SIGKILL` is outside the `local-capability-v2` boundary. `SIGKILL`
before cleanup can leave raw data at its path; `SIGKILL` after the raw path is
unlinked but before group teardown and reap can leave the pathless child
process group alive. A descendant can also escape process-group containment by
creating another session. `production-v2` therefore requires a protected
cgroup/job supervisor, or an equivalent descendant-lifetime boundary, that
owns cleanup independently of the runner.

The snapshot supervisor supplies each controller input as an exact numeric
`/proc/<ancestor-pid>/fd/<n>` path together with its owner PID/starttime and
Linux device, inode, mode, and size. The controller opens a pidfd before making
any owner claim, requires the owner to be a live same-UID ancestor, opens the
memfd once, and compares the opened object with that supervisor-provided
identity. It repeats liveness, ancestry, UID, and starttime checks after open
and after read while the pidfd remains live. The content digest remains an
additional byte binding; it is not used as a substitute for descriptor-object
identity.

This local boundary does not defend against root or a same-user process able
to ptrace, inject into, or otherwise control the source-state supervisor,
runner, debugger, or checker. Those actors can alter the measuring process
itself and remain outside the pilot threat model.

The ROCgdb and LLVM tools remain fixed, canonical executable paths selected
from the local ROCm installation. This pilot does not bind those tool images to
administrator-installed digests or execute them from retained descriptors, so
their installation is platform-trusted. The pinned Git and cancellation
hardening do not upgrade the archive to production evidence.

The normalized transcript is segmented around every debugger continuation.
The checker requires an exact BP2 line-69 hit followed by an AMDGPU-wave frame
and all physical argument observations before BP3 is armed. It then requires
an exact BP3 line-70 hit, another AMDGPU-wave frame, and local `i` before the
final continuation. ROCgdb runs with `--return-child-result`. Only after the
hardware test exits normally and ROCgdb returns its zero child status does the
runner append one `FE2O3_S09_HARNESS_RESULT_V1` marker carrying the HSACO
digest, a runner-generated 256-bit nonce, and `result=passed`. The checker
requires the normal inferior-exit record before this marker and zero ROCgdb
status after it. It does not depend on Cargo status-line adjacency and accepts
bounded debugger thread-exit interleaving, then requires normal inferior exit,
the runner marker, debugger hardware-pass marker, and zero ROCgdb exit status
in that order. Removing, moving, or forging the marker fails closed, as do
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

The exact 18-field semantic order is `schema`, `crate`, `module`,
`logical_name`, `export_name`, `profile`, `source_path`, `source_sha256`,
`source_bytes`, `target`, `target_capabilities`, `code_object_version`,
`rustc_opt_level`, `rustc_debug_info`, `injected_debug_policy`, `abi_sha256`,
`launch_sha256`, and `portable_mir_sha256`. The exact 20-field build order is
`schema`, `semantic_claim_sha256`, `cargo_metadata_sha256`, `crate_binding`,
`kernel_binding`, `observed_def_path`, `observed_symbol`,
`rustc_mir_capture_sha256`, `prepared_rustc_command_sha256`,
`rustc_executable_sha256`, `cargo_fe2o3_executable_sha256`,
`declared_cargo_executable_sha256`, `pinned_cargo_image_sha256`,
`observed_parent_pid`, `observed_parent_start_time_ticks`,
`codegen_backend_sha256`, `worker_config_sha256`,
`worker_executable_sha256`, `worker_build_identity_sha256`, and
`llvm_build_identity_sha256`.

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
form the exact closed alpha-only physical set, and the semantic export must
resolve to that member. This structural binding does not itself authenticate
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

The intended GPU-gated local capability invocation is:

```text
FE2O3_ALLOW_S09_DEBUG=1 \
FE2O3_LLVM_LINK_WORKER=/absolute/fe2o3-llvm-link-worker \
FE2O3_LLVM_LINK_WORKER_BUILD_ID=<measured-worker-id> \
FE2O3_LLVM_BUILD_ID=<measured-llvm-id> \
FE2O3_S09_EVIDENCE_DIR=/absolute/new-evidence-directory \
  scripts/ci-local.sh s09-debug-hardware
```

The compile portion performs the genuine direct LLVM/LLD alpha-only build and
derives both identity records exclusively from the emitted HSACO. The
alpha-only controller allows the complete local invocation to run native
ROCgdb, emit `s09-evidence-manifest-v2.tsv`, and validate a capability bundle.
Its `trust_domain=local-capability-v2` still prevents promotion: local
selection of the checkout and host executable is not an evidence-grade
provenance boundary. A future protected controller must select immutable
inputs and the GPU runner, then install a separately measured `production-v2`
manifest and policy. Generic CI continues to run field-by-field mutation,
missing, zero-digest, duplicate, reorder, truncation, oversize, trailing-data,
unknown-field, deterministic-serialization, exact alpha-only artifact,
digest-substitution, and lane-guard tests; neither those tests nor a local
pilot archive can satisfy production debug evidence.

Pilot evidence classes are:

- `unit`: metadata injection, source-shape, and checker mutation tests;
- `ui`: closed Worker V2 configuration admission and rejection diagnostics;
- `compile`: genuine rustc/LLVM/LLD COV6 emission plus DWARF verification; and
- `debug`: normalized `llvm-dwarfdump` and native ROCgdb archive inspection.

This pilot is not production parity evidence. S09 remains `Missing` until a
production debug result is signed and accepted by the parity gate.
