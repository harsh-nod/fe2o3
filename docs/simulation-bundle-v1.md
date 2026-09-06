# Source-to-simulator bundle V1

`fe2o3-export-sim` turns an ordinary admitted `#[kernel]` crate into one
bounded, content-addressed CPU-simulation input without requiring a GPU. It is
an explicit extraction command. It does not run the kernel and never falls
back from a failed hardware launch.

```text
fe2o3-export-sim \
  --crate my_kernel_crate \
  --output "$PWD/my-kernel.fe2sim" \
  --bundle-version 6 \
  --target gfx942 \
  -- --package my-kernel-package --lib
```

The example explicitly selects the current V6 envelope. Omitting
`--bundle-version` preserves the V1 default and its existing wire bytes.
`--bundle-version 2` selects a distinct envelope and Source Map V2 route. For
the broadly admitted kernel fragment, this opt-in route captures bounded rustc
lexical scopes from the same extraction session and maps exact one-to-one
kernel parameters to their exact KIR function values with a nonzero
function-wide generation only when MIR never moves, drops, storage-resets,
mutates, or mutably aliases the entry value.
Non-parameter locals, projected or constant debug values, and composite ABI
cases without a one-to-one KIR parameter remain typed `Unrepresented`; the
compiler does not infer temporary SSA lifetimes. Invalid, control-containing,
or over-bound debug names reject V2 rather than being changed. V1 does not
inspect this V2 metadata. See the production tests
`ordinary_kernel_sources_export_and_query_exact_v2_source_variables` and
`v2_rejects_an_overbound_debug_name_without_inspecting_it_on_v1`.

`--bundle-version 5` selects an independent self-contained envelope rather
than nesting or changing V1-V4. V5 binds exact production KIR V8 or V9 bytes,
an exact structural same-module KIR V10 re-encoding, Source Map V2, semantic
MIR, and both storage maps. V8/V9 normalization must round-trip through its
original encoder byte-for-byte. Any version-specific or lossy drift rejects
export or admission. The ordinary-Rust gfx950 f32 wave collective path is the
first production V9 case on this route. Production lowering does not yet emit
V10-only memory intrinsics, so they remain typed producer-unavailable.

`--bundle-version 6` selects the current additive self-contained envelope. V6
binds exact verified canonical KIR V11, Source Map V2, semantic MIR, source
lineage, and both storage maps without changing any V1-V5 bytes or decoders.
V11 is required for the explicit same-pointee, same-address-space
`ReadWrite`-to-`ReadOnly` pointer restriction used by retained shared borrows.
The restriction preserves pointer identity; it does not prove Rust aliasing or
lifetime rules.

The command requires the repository's pinned nightly toolchain, its `rust-src`
component, and the AMDGPU Rust target. `--crate` is rustc's crate name, which
normally replaces package-name hyphens with underscores. Arguments after `--`
are Cargo package, feature, or target-kind selection; `--target` and
`--target-dir` are owned by the exporter and rejected there. Cargo `--config`,
`--release`, and `--profile` overrides are also rejected so they cannot replace
the fixed semantic extraction profile.

The command invokes Cargo `check` with `fe2o3-rustc-extract` as the workspace
wrapper. Dependencies pass through. The selected primary crate enters
extraction-only custody and shares the production source/MIR/KIR stages:

```text
admitted rustc collection
  -> semantic MIR owner
  -> ranked generic checks
  -> target-neutral KIR lowering
  -> exact verified KIR V7 projection (Bundle V1-V4)
     or exact same-module KIR V10 encoding (Bundle V5)
     or exact verified KIR V11 custody (Bundle V6)
  -> authority-free simulation bundle
```

The exporter owns a semantic extraction Rust flag profile. It preserves the
selected crate's pre-inlining MIR owner and disables jump threading; forced MIR
inlining is explicitly disabled because it can replace the source-level proof
structure before the semantic importer runs. Optimization level zero, target
CPU, and wave width remain fixed independently of the caller's Cargo profile,
configuration, Rust flags, or wrapper environment.

There is no second MIR importer or KIR lowerer. Extraction consumes the
transaction immediately after target-neutral KIR verification, before formal
memory admission, target LLVM lowering, protected compiler-module publication,
linking, loading, or launch. It is not a protected publication occurrence. The
output is created new with mode `0600`; an existing path, including a symlink,
is rejected.

Every bundle version is a content-bound, authority-free simulation input.
Source Map V2 does not authenticate compiler execution or establish source
refinement, proof, artifact, hardware, load, launch, or performance authority.

## Simulate and debug the bundle

Prepare the same strict request document used by the raw KIR simulator, then
consume the bundle directly:

```text
fe2o3-kir-sim \
  --bundle "$PWD/my-kernel.fe2sim" \
  --request "$PWD/request.json"

fe2o3-debug sim \
  --bundle "$PWD/my-kernel.fe2sim" \
  --request "$PWD/request.json" \
  --protocol jsonl
```

For Bundle V5, use `--bundle-v5` in both commands. That spelling is a distinct
admission route and does not reinterpret `--bundle` or raw `--kir-v7`.

For Bundle V6, `fe2o3-kir-sim` uses the separate `--bundle-v6` route. The
library-level `load_debug_simulation_bundle_v6` admission API retains the same
exact bundle/KIR/target relationship for compiler conformance tests. The
public `fe2o3-debug sim` CLI has not yet added a V6 spelling; use of its V5
route must not be described as V6 custody.

Each available route uses the existing hardened regular-file boundary, rejects
oversize/change/substitution, strictly decodes and revalidates the complete
bundle, maps its exact target, and admits only its embedded verified KIR
version: V7 for legacy bundles, V10 for Bundle V5, and V11 for the standalone
simulator's Bundle V6 route. The tools do not recompile, re-lower, load, launch,
or fall back to a different route. The simulator's `--bundle`, `--bundle-v5`,
`--bundle-v6`, and raw `--kir-v7` inputs are mutually exclusive.

The compiler emits a bounded debug map from rustc spans observed in the same
extraction transaction. `fe2o3-debug` passes the exact payload, verified
simulation-bundle subject, and committed map identity through the
compiler-bundle admission API. Source locations are then
labeled `compiler_bundle_bound`, which means exact content association, not
protected compiler-execution authentication. The map wire contains only its
compile-time bundle subject and KIR identity. Request and logical wave width
remain runtime inputs; their configuration identity is derived internally and
rechecked before the map is bound. Bundle session identities also commit the
verified bundle subject and therefore its exact target.

Map construction uses the retained semantic MIR owner and
`SemanticKirCorrespondenceV1`; it does not rerun rustc, lower a second IR, or
infer locations from paths. Every statement and terminator correspondence
range is mapped to its KIR operations. A source construct with a zero-operation
range is retained in `eliminated`, including rustc's valid zero-width spans.
Operations classified as synthetic by the lowering correspondence have no
fabricated source location and remain typed unavailable in the debugger.

V1 serializes rustc's resolved call site only. Macro expansion-chain identity
remains in semantic MIR but is not exposed as an expansion stack. Display paths
use rustc's remapped spelling and are inert labels: source bytes are never
reopened, and the map grants no path or filesystem authority. File length,
span ranges, lines, and columns are captured from the live `SourceMap` before
rustc custody ends.

## Exact identities and bounds

Bundle V1 is a strict binary wire with a hard maximum of the 16 MiB KIR bound,
the 4 MiB debug-map bound, an optional exact 690-byte inert compiler-execution
subject association, and a small fixed header/target allowance. Its simulation
bundle subject identity commits:

- an explicit compiler-execution binding state;
- the exact domain-separated V3 inert receipt identities and preimage lengths
  for rustc's compiler-owned identity-inventory and preflight-plan transcripts;
- the production canonical KIR V8 identity and length;
- the exact verified canonical KIR V7 identity and length;
- the `gfx942:xnack-` or `gfx950:xnack-` target admitted by the live rustc
  session;
- a deterministic projection of every kernel name, entry, launch domain,
  workgroup size, parameter type, and result type.

The extraction command always emits
`SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly`. The other
wire variant retains a claimed subject identity and all 690 claimed canonical
bytes. It is intentionally an unverified association: bundle decoding commits
those bytes but does not validate the nested subject. A higher owner must
strictly decode `InertCompilerExecutionSubjectV1`, independently reconstruct
the subject from an already-retained or consumed exact strict V3 handoff, and
cross-check the source receipts, KIR, target, and transaction before promotion.
That same move-only production join is not implemented by this extraction
path.

The complete bundle identity additionally commits the optional debug-map
bytes. Decoding re-verifies canonical V7, independently re-encodes V8 from the
same semantics, rederives the kernel ABI and both identities, and rejects
truncation, trailing bytes, unknown flags, zero identities, and substitution.
`VerifiedSimulationBundleV1::canonical_kir_v7()` exposes the exact bytes used
by direct `--bundle` admission in `fe2o3-kir-sim` and `fe2o3-debug`.

A valid bundle proves exact local bundle content. It does not authenticate
compiler execution, source authorship, or provenance and grants no proof,
artifact, compiler, load, hardware, or launch authority. The canonical subject
and current signature protocol alone authenticate nothing; a signature-verified
receipt under that protocol authenticates only the policy-pinned signing key,
not protected compiler execution. This extraction path does not consume or
join protected issuer and durable Worker V3 evidence, so that property remains
false even when those independent protocols are available.

## Version and debug-map boundary

Production currently retains canonical KIR V8 for ordinary kernels and V9 for
the gfx950 attention surface. Simulator bundle V1 projects the already-lowered
V8 module through the frozen V7 encoder. If any semantic field cannot round
trip through V7, export fails. V9 is rejected with a typed diagnostic; it is
never silently downgraded.

Bundle V5 instead retains the exact V8/V9 production identity and embeds its
exact verified V10 same-module encoding. Its independent header and section
identities bind the target, ABI, source lineage, source map, semantic MIR, and
storage correspondence. V1-V4 canonical bytes and decoders remain unchanged.

Bundle V6 retains exact verified KIR V11 directly and updates each embedded
source/storage binding to that V11 identity. Its independent envelope is the
production export route for modules that contain pointer-access restriction.
V1-V5 remain strict historical compatibility formats and are never relabeled
or silently upgraded.

The bundle reserves a separately bounded `fe2o3-debug-source-map-v1` payload
and exposes a non-circular simulation bundle subject identity for that map to
bind. The compiler prepares the map-independent subject, constructs the map
against that typed prepared state, and finalizes the bundle once; loose claimed
hashes are not used to break the cycle. Embedded maps use one compact canonical
JSON encoding. The low-level caller-bound sidecar decoder accepts other strict
JSON whitespace/order while committing its exact bytes. Unknown, duplicate,
null, oversize, stale-subject, stale-KIR, and substituted map inputs fail closed.

Current `SemanticKirCorrespondenceV1` identifies KIR block and operation ranges
but not a KIR function identity, and its production lowering currently admits
one body. Map construction therefore fails closed if a module has zero or
multiple KIR bodies. Helper-body source maps require the correspondence owner
to add exact KIR function identity; V1 does not claim helper coverage before
that change. Source-variable reconstruction and macro expansion stacks also
remain future work.

## Current UX boundary

This first command is the production extraction driver packaged as a direct
binary. It is not a `cargo fe2o3 simulate` route: that deleted route selected a
qualification oracle and performed a second MIR-to-KIR lowering. A future
`cargo fe2o3 export-sim` spelling may wrap this command, but must preserve the
same selector-free transaction and authority boundary.
