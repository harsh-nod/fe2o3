# Assembly authoring: first implementation slice

Tracking: #280 (instructions/regions), #281 (resource views), #282 (multi-level
authoring). This is a prerequisite slice, not completion of those umbrellas.

The [milestone status and owner handoffs](assembly-authoring-implementation-status.md)
retain the full scope of all three issues and distinguish remaining implementation
from shared compiler dependencies.

## One program, several observations

The executable subject remains the ordinary source-to-MIR-to-canonical-KIR
pipeline. A simulation bundle is a bounded, content-bound observation of that
program, not a compiler resume token, source authenticator, or launch permit.
The new read-only authoring queries consume the exact Bundle V6 / KIR V11 body
and its embedded Source Map V2. They do not decode a second authoritative IR.

| Surface | This slice | Still unavailable |
| --- | --- | --- |
| Instruction contract | Existing gfx942 integer subset, shared operand/effect validation | Broad ISA, explicit allocation, scheduling and exact encodings |
| CPU execution | Six VGPR integer operations, exact 32-bit result bits | SGPR uniformity, EXEC/VCC/SCC state, instruction latency |
| Lowering | Existing LLVM inline-assembly path uses the shared contract | Direct assembler backend and final-code exact-contract checks |
| Debugger | Explicit `sim --bundle-v6` admission with embedded source binding | Live physical register/resource-state capture |
| Inspection | Bounded immutable source-bound snapshots and region boundaries | Mutable graph editor and production resume from snapshots |
| Source candidate | Six typed u32 markers enter the real source exporter; selected generated-helper variants freshly export and pass an independent CPU case | General source replacement and final ranked/protected artifact admission |
| Resource display | Exact stopped CPU checkpoint bytes/initialization and bounded allocation/access pages | Allocation-lifetime inference, physical register values, GPU timings |

## Integer contract

The shared kernel-IR validator admits `v_mov_b32`, `s_mov_b32`, `v_add_u32`,
`v_sub_u32`, `v_and_b32`, `v_or_b32`, and `v_xor_b32` for the existing gfx942
lowering profile. Operand roles, register class, single i32/u32 result, exact
input types, complete source-identity shape, and NoMemory/effect-free contract
are checked. Nonzero source identities are not source authentication.

CPU simulation admits the six **vector** operations only. Add/sub wrap modulo
2^32; an i32 carrier preserves those same bits. The scalar-register move remains
unsupported because per-invocation scalar values do not establish SGPR uniformity.
The simulator retains existing instruction checkpoints and memory checks; it
does not emulate physical allocation or fabricate register observations.

Negative cases include wrong targets, operand roles/constraints/types, missing
source identity fields, declared memory effects, unsupported instructions,
stale snapshot selectors, oversized requests and inconsistent resource snapshots.
Existing surrounding KIR checks still own memory bounds/permissions,
initializedness, control flow, races and synchronization. Passing these checks
does not establish unmodeled hardware behavior or a universal refinement proof.

## Inspection command

After building `fe2o3-source-isa-observation --bin fe2o3-author`, pass exact
canonical Bundle V6 bytes on stdin:

```sh
fe2o3-author inspect < kernel-v6.fe2sim
fe2o3-author operations --bundle-identity HEX --start 0 --limit 16 < kernel-v6.fe2sim
fe2o3-author select --selector '{"bundle_identity":"HEX","canonical_kir_digest":"HEX","target":"gfx942:xnack-","operations":[{"function":0,"block":0,"operation":0}]}' < kernel-v6.fe2sim
fe2o3-author materialize --selector 'EXACT_SELECTOR_JSON' --helper compute < kernel-v6.fe2sim
```

`HEX` and operation coordinates must come from the actual snapshot. Ordinals
are local to that immutable snapshot, never persistent cross-build anchors.
Selections are contiguous in one block; live-in/live-out analysis includes the
containing function's uses and terminators. Queries and emitted source have
explicit bounds. Materialization returns JSON containing a diagnostic source
candidate, never edits a file, runs a compiler, or launches a kernel.

### Preview an explicit helper insertion

For a supported u32 bitwise or typed-ISA selection, request a diagnostic proposal to
append its complete named helper to an existing UTF-8 Rust file:

```sh
fe2o3-author preview-helper-insertion \
  --selector 'EXACT_SELECTOR_JSON' --helper compute \
  --source src/kernel.rs --expected-source-sha256 EXACT_FILE_SHA256 \
  < kernel-v6.fe2sim
```

The source path must be an explicit normalized relative `.rs` path. The command
reads that regular file once, with a 1 MiB limit, and checks the caller's SHA-256
against its exact bytes. It rejects symlinks, unsafe paths, stale content, missing
or ambiguous source spans, and unsupported materialization. Source-map display
paths are inert labels and are never opened. The ordinary-source fill example
currently rejects because its selected operation has no typed-ISA materializer.

Output is proposal JSON containing the original and proposed file hashes, exact
selection, EOF insertion range, and generated helper. The original source prefix
is preserved byte-for-byte in the proposed edit; replacements are unavailable.
No file is written and no compiler or kernel is run. A source-map file identity
does not hash source contents, so the proposal explicitly records an unverified
source association and unavailable semantic application. A safe Rust insertion
boundary, source admission, equivalence, and final machine contracts have not
been established by producing this text preview.

### Explicitly create a new text candidate

On Linux filesystems supporting `O_TMPFILE` and procfs descriptor links, a
separate action can publish those proposed bytes to a **new** relative `.rs` path:

```sh
fe2o3-author create-source-candidate \
  --selector 'EXACT_SELECTOR_JSON' --helper compute \
  --source src/kernel.rs --expected-source-sha256 EXACT_FILE_SHA256 \
  --expected-proposal-sha256 EXACT_PREVIEW_OUTPUT_SHA256 \
  --candidate src/kernel_candidate.rs < kernel-v6.fe2sim
```

Save and review the preceding `preview-helper-insertion` output first. Hash its
exact compact JSON including the final newline. The creation command does not
deserialize that report as authority: it reselects the bundle region, rereads
the explicit source, regenerates a private proposal and compares this digest.

Publication uses a fully written, synced anonymous file and an atomic no-replace
link in the retained destination directory. Existing files, directories, hard
links and symlinks reject. Parent components must be ordinary directories, not
symlinks. The original source is never written. Pre-publication failures leave
no named temporary file; a post-publication error can leave the new candidate
in place, with no automatic rollback or undo. Unsupported filesystems reject.

The receipt records source/proposal/candidate hashes and candidate device/inode
as diagnostic facts. Rechecking covers retained file bytes and its name in a
retained parent at a point in time, not a source compare-and-swap or a promise
that ancestor paths cannot move. The candidate is the original bytes followed
by the complete helper, with owner-only read/write permissions. It does not
replace a kernel call, establish a compiled variant, or prove EOF insertion is
valid Rust. Its source-root context may need explicit integration. Review the
diff and freshly compile integrated source through the normal frontend; this
command performs no compiler, kernel, proof, load or launch action. Removing
the candidate is a separate explicit user action; the original needs no undo.

## The source-admission dependency

### Inspect LLVM text without minting an artifact

The diagnostic example uses the existing Bundle V6 decoder, exact gfx942 target
binder and complete-module LLVM lowerer:

```sh
cargo build --locked -p fe2o3-amdgcn-model --example inspect_bundle_v6_llvm
cargo run --locked -p fe2o3-amdgcn-model --example inspect_bundle_v6_llvm -- kernel-v6.fe2sim
```

Its comment header identifies the original neutral KIR and separately identifies
the target-bound KIR. Typed instructions and helper definitions/calls remain in
the emitted LLVM text. All lowering succeeds before output begins. This does not
invoke an assembler, decode final instructions, grant production resume, or
produce a verifier receipt or HSACO. Source and transcript IDs remain inert
observation references. After running the source and helper smokes below, check
their exact templates, constraints and source occurrences with:

```sh
llvm_run=$(mktemp -d)
node scripts/assembly-source-llvm-inspection-smoke.mjs \
  "$assembly_run/capture" "$roundtrip_run/capture" "$llvm_run/capture"
```

### Reproduce the supported ordinary-source path

Build `fe2o3-rustc-extract` and `fe2o3-export-sim` from `rustc-codegen-fe2o3`,
`fe2o3-author` from `fe2o3-source-isa-observation`, `fe2o3-kir-sim` from
`fe2o3-kir-sim-cli`, and `fe2o3-debug` from `fe2o3-debug-cli`, using the pinned
toolchain. Then run:

```sh
authoring_run=$(mktemp -d)
node scripts/authoring-v6-smoke.mjs "$authoring_run/capture"
```

The script exports actual Rust `examples/fill/src/lib.rs`, inspects/selects its
eight lowered operations, rejects a stale selector and unsupported promotion,
checks four independent 42.5f32 output words plus untouched canaries, and captures
a source-bound memory window through the V6 debugger route. It requires a fresh
output directory and retains its exact requests, responses and hashes.
`crates/fe2o3-source-isa-observation/tutorial/fill-v6` retains the actual exported
bundle for CLI regression tests; this is not a manufactured instruction fixture.

The companion site previews are `docs/inspect-lowered-kernels.md` and
`docs/resource-memory-windows.md` in `fe2o3-kernels`. They do not change existing
curriculum pins or claim complete production round-trip qualification.

### Reproduce actual typed-instruction source admission

The compiler now recognizes the six closed gfx942 **u32**
`fe2o3_device::amdgpu_asm!` markers in semantic MIR V30. This profile is VGPR32,
NoMemory, no declared effects, and one authenticated kernel root with its
supported helper closure. Arbitrary Rust `InlineAsm`, scalar-register operations,
physical allocation and extra assembly options remain unsupported.

The complete standalone fixture is
`crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30/src/lib.rs`,
with its adjacent manifest and local lockfile. After building the exporter,
authoring CLI and simulator with the pinned toolchain, run from the compiler:

```sh
assembly_run=$(mktemp -d)
node scripts/assembly-authoring-v30-smoke.mjs "$assembly_run/capture"
```

Its typed kernel has a required 64-lane workgroup and checked `DisjointSlice<u32>`
write. Two moves feed add, subtract, xor, and, and or operations. The `edited`
Cargo feature changes the final OR operand from 256 to 512: an intentional
computation change, not a semantics-preserving schedule claim. For
`a = 0xfffffff0`, `b = 0x25`, independent wrapping-integer oracles require four
outputs of 469 (base) or 725 (edited), plus two unchanged canary words.

The script exports both variants from real Rust, requires all six instructions
and seven distinct occurrence references, selects the final OR and retains its
typed draft. Read `base-operations.json`, `base-selector.json`, `base-draft.rs`,
their edited counterparts, and `receipt.json`. Unit, statement, semantic-MIR,
KIR and bundle identities change; the unchanged declaration keeps its contract
digest. This smoke does not compile the generated draft itself, so it is not
evidence of a successful generated-helper round-trip.

Occurrence references bind compiler-observed MIR/source origins, retained bodies
and root contracts. They are not raw source-file hashes or whole-build/toolchain
receipts. Decoded reports continue to label these references inert.

### Freshly compile the generated helper and an explicit instruction edit

After the preceding source smoke succeeds, use its retained baseline as input
to a separate generated-helper exercise. The second directory must be new:

```sh
roundtrip_run=$(mktemp -d)
node scripts/assembly-source-roundtrip-smoke.mjs \
  "$assembly_run/capture" "$roundtrip_run/capture"
```

The script materializes the exact selected final OR as
`assembly_promoted_region`, then creates two standalone source crates. It
explicitly replaces the one known fixture expression with
`assembly_promoted_region(low, 256).0` and appends the complete generated helper.
The second candidate changes only that helper's `v_or_b32` marker to
`v_and_b32`. These are named concrete candidates, not an inferred source
replacement API; the original fixture remains byte-for-byte unchanged.

Both candidates have successfully passed fresh Rust export to Bundle V6. The
retained KIR includes the helper call, seven assembly operations and distinct
root/helper function references. The unchanged helper yields four words of 469;
the intentional OR-to-AND edit yields four words of 0 for the same independent
integer test case. Both canary words remain unchanged. Bundle, KIR, semantic-MIR
and preflight identities are fresh for each variant; no edited snapshot is
accepted as an executable.

Review `generated-helper.rs`, `no-edit-source-change.json`,
`edited-instruction-source-change.json` and each candidate's `src/lib.rs` before
reading its operations and simulation results. `receipt.json` records the exact
source and bundle hashes and the checks performed. This establishes the exercised
source-admission and CPU case only, not universal equivalence, final ranked
verification, protected artifact admission, exact machine code or GPU execution.
Ordinary-Rust bitwise promotion is a separate exercise; this one starts with an
existing typed-instruction source operation.

### Promote one ordinary Rust bitwise expression

The standalone fixture
`crates/rustc-codegen-fe2o3/tests/fixtures/ordinary-bitwise-promotion-v1/src/lib.rs`
starts without ISA markers: `let low = (a ^ b) & 255; let result = low | 256;`.
Its full source retains the typed kernel contract and checked output write.
With the same tools built, run:

```sh
bitwise_run=$(mktemp -d)
node scripts/ordinary-bitwise-promotion-smoke.mjs "$bitwise_run/capture"
```

This three-export exercise has passed from actual source. It selects the one
canonical u32 `Binary/BitOr` by its exact snapshot coordinate; that observation
retains a null mnemonic and no assembly source reference. The materializer
emits `bitwise_promoted_region`, and explicit fixture integration calls
`bitwise_promoted_region(low, 256).0`. Fresh source compilation introduces one
typed OR instruction in the called helper; the surrounding XOR and AND stay
ordinary Rust. A second candidate edits only the helper's OR marker to AND and
freshly compiles again, without retaining the old OR as an executable fallback.

The independent CPU oracle checks four words of 469 for both ordinary source
and the unchanged helper, and four words of 0 for the deliberate instruction
edit, with both canaries unchanged. All three exports have distinct bundle,
canonical KIR, semantic-MIR and preflight identities. The original source remains
unchanged. Inspect `ordinary-operations.json`, `ordinary-selector.json`,
`generated-helper.rs`, the two explicit `*-source-change.json` files, complete
candidate source crates and `receipt.json`. This is one checked differential
case and a concrete source-integration example, not a universal equivalence
proof, arbitrary-source replacement, final artifact admission or hardware result.

### Fresh source compilation is not final artifact qualification

The source exporter and CPU oracle exercise the source/MIR/KIR path. They do not
establish final ranked verification, exact machine encodings, protected artifact
admission or hardware execution. The generated-helper smoke exercises source
correspondence; final-admission owners remain separate, and neither fixture
bypasses them. Replacing an authored instruction with ordinary
arithmetic to avoid a failed check would lose its instruction-selection contract
and is not an acceptable shortcut.

The eventual loop is source -> checked lowering -> explicit source promotion ->
fresh source compilation. Editing does not reuse prior proof receipts or captures.
Restoring retained high-level source is distinct from lifting edited assembly;
arbitrary assembly has no promised inverse into its original Rust.

## Independent remaining milestones

Complete assembly regions, explicit register/LDS ownership, waits/barriers,
target expansion and final-machine correspondence remain #280 work. Register/LDS
lifetimes, physical access/transaction views, tile/MFMA mappings and live GPU qualification remain
#281 work. Semantic source replacement, complete production round-trips and
checked schedule recipes remain #282 work. New-candidate text conflict checks
do not establish those missing semantic guarantees. A CPU replay schedule is not a compiler
schedule recipe. Tutorials must label each stage separately and must not replace
the existing source/evidence inventory or publication pins.
