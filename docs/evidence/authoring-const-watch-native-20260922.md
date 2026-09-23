# Const-specialized helpers, watch/source replay and native inspection

This 2026-09-22 diagnostic increment advances #280 M2/M5/M6, #281 V2/V3/V5
and #282 U2/U4. It does not complete those broad milestones. M1, V1 and U1
remain accepted (3/18); the original
[contract review](../assembly-authoring-contract-review-20260922.md) still applies.

## Implemented author workflow

The additive [const-u32 materializer](../../crates/fe2o3-source-isa-observation/src/multilevel_authoring_const_u32_v1.rs)
and normal `fe2o3-author materialize-const-u32` command produce a bounded Rust
helper for one retained u32 AND/OR/XOR operation. Exactly one operand must have
an earlier direct same-block `Constant::U32` definition. The immutable owner
supplies the bits; the caller cannot assert them. The second operand remains a
runtime parameter. Existing materializers, schemas and commands are unchanged.

A generated identifier such as `v14` is a Rust value name, not physical VGPR 14.
The helper has a const-generic parameter and uses the existing typed instruction
macro. Authoring a new source variant and compiling it through the normal frontend
remain explicit steps. This is not arbitrary KIR editing or a compiler-resume token.
LLVM IR remains in the ordinary pipeline; typed assembly is lowered through its
existing instruction/inline-assembly path, not a replacement LLVM backend.

The [fresh-source driver](../../scripts/const-u32-helper-source-smoke.mjs) checks
ordinary baseline, generated helper<256>, edited helper<512>, exact repeated source
and two concrete specializations in one kernel. Each fork independently passed
five normal exports, 169 stages and 150 complete-buffer/canary CPU checks.
The final formula changes intentionally when the specialization changes.
Actual typed helper bodies are retained, but current public operation observations
do not expose exact call targets; no kernel-to-helper edge is guessed from names.

Canonical const source receipt: 417970 bytes, SHA256
`8c6411c8d2d91fb406607312aba1095a2d2b5cf787c4eb3bc365cccca4261fda`.
Mirror: 424517 bytes,
`83a187a5f151e8e64abd6420ec62019ffe93a2431a189c9998b86e8e0f7037ee`.
Both record 455 selected input pins. See the
[const-helper tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/const-u32-helper-promotion-v1.md)
for exact API, source edits, independent oracles and unsupported cases.

## Real watchpoint/source replay

The new [capture driver](../../scripts/resource-watch-source-values-v2-smoke.mjs)
uses the unchanged normal assembly-authoring fixture and public CPU debugger.
It retains all 41 original request/response pairs, the unchanged seven-pair
watchpoint excerpt and a disjoint 27-pair source excerpt.

| Recorded moment | Event / revision | Logical scope | Available source / SSA |
| --- | --- | --- | --- |
| Exact watch stop | 32 / 3 | Uncaptured | Both unavailable |
| Immediate post-write | 33 / 4 | lane 0 | Source unavailable; 18 SSA rows |
| Next invocation before its first operation | 34 / 5 | lane 1 | 12 source bindings; 3 SSA rows |
| Reverse before the lane-0 store | 31 / 6 | lane 0 | 12 source bindings; 18 SSA rows |
| Repeated later checkpoint | 34 / 7 | lane 1 | 12 source bindings; 3 SSA rows |

The first two source queries genuinely return `checkpoint_not_captured`.
The later controls retain exact counts 1/2/2; neither steps nor scopes are rewritten
to fit the old importer. Each complete source query has six pages, with a/b
captured and ten other bindings explicitly not represented. Source and SSA remain
separate observations. The global memory window restores on reverse and repeats
on forward; a separate full simulation checks four final words of 469 plus canary.
Three real stale/invalid-cursor errors leave session state unchanged.

Capture receipt: 22064 bytes,
`d6d58a1d6acf73921e7b475d8e24fef8eea6ce55a90c0982d30243156fbdb6a1`;
outer receipt: 36572 bytes,
`1d69ab267ecd033a76789d0058c6db11854abf4d65c04f692e8b3629d92977b9`.
The new site panel imports the seven original local files, verifies bounded byte
hashes and exact full-session/excerpt correspondence, and reuses existing
resource/source/SSA/memory renderers. It performs no network request, persistence,
debugger action or source mutation. All seven primitive inputs are copied before
asynchronous hashing; changed files, cancelled work and superseded imports clear
old displayed state. Hash consistency is not producer or source authentication.

See the [watch/source tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/resource-watch-source-replay-v2.md).
Frame depth and the legacy occurrence field do not identify dynamic activations;
allocation generation/reuse, physical registers and general fault coverage are
not supplied by this bounded example.

## Reproducible final-native inspection

The new standalone
[observer directory](../../tools/fe2o3-instruction-native-observation/README.md)
builds against explicit unchanged worker sources and the reviewed static LLVM/LLD
component SDK. No root build registration, production worker, ABI, proof,
finalizer or admission route changes. Exact source-size/hash, canonical-path,
version/build-claim, graph, provider and output-directory guards fail closed.

Fresh default/edit/repeat source exports passed 100 stages and 90 CPU checks;
the new observer produced default/edited O0/O3 code objects. The
[shipped join](../../scripts/instruction-edit-native-join.mjs) independently checked
294 pins / 137299031 bytes, all four complete HSACOs and exact instruction/descriptor
file offsets. Declared VGPR high-water 6 is distinct from encoded capacity 24 at
O0 and 8 at O3. Neither counts nor selected instruction bytes establish allocation
lifetimes, occupancy or whole-kernel native correctness.

Fresh join: 140457 bytes,
`66edd64c990d2305affe058c9007c97cd0649879ce3c68ad3bbff710b9717bbc`.
Twelve fresh configure-only controls reached their exact expected first diagnostic;
the complete original SDK and selected inputs were unchanged. Shape controls
separately passed 4 positives/48 negatives, identity 1/3 and file-stat 1/9.
The [complete provenance](../../tools/fe2o3-instruction-native-observation/SOURCE_PROVENANCE.md)
records actual new source/LLVM/binary/payload hashes, commands and retained failures.
No mirror-native or repeat-native execution is claimed.

The separate [final-native viewer tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/final-native-comparison-v1.md)
opens a 14-artifact capsule of the original retained Phase19 run, not a relabeled
Phase20 capture. It verifies whole imported payload bytes and selected exact
offsets, distinguishes original/edited variants and leaves the earlier CPU view
unchanged. The viewer is not an ELF decoder, proof checker or hardware debugger.

## Qualification and limits

Compiler bases were `bcb4dbb77cd4d944feec1d215c4b2963f6fabb73` and mirror
`7f52c36dde9ec027f6a95828bb43ada08e1a230e`; site base was
`e91a2a57aaffc9c4f1104ab4c33ce003e353b7a1`. These were working candidates,
not claims that a base commit alone contains the changes.

Both independent compiler builds passed 145 authoring-package tests and built
the normal backend/exporter/simulator/debugger tools. Backend regressions passed
1711 canonical / 1710 mirror tests, with 110 ignored in each and zero failures.
Both forks passed 108 relevant Node controls. Pinned rustfmt checks and strict
Clippy for the authoring library and normal author CLI passed. This is not an
all-workspace lint result: the separate historical backend Clippy run retained
31 existing diagnostics and has not been relabeled green.

The fresh watch/source and instruction-source/native gates used the exact
6561-file / 99789345-byte compiler
census `e1d48a1a29dc3c44e1746deae2db4032ede02ae330017284ab3378e0ab1fdaa6`.
Earlier build/configure gates and later documentation have separately retained
censuses. Executables and original evidence remain frozen. Site qualification
is recorded separately in the
[companion validation record](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/const-watch-native-qualification-20260922.md).

Failures remain retained: an invalid test fixture was corrected to valid typed
IR before testing the intended refusal; an exporter check exposed prototype-only
object equality and gained exact-field regression coverage; the first normal
LLVM launcher could not load dynamic Rust std and was rebuilt with static std
from the same example; a native preflight rejected an outdated source census;
and a test-only readonly deletion failed TypeScript before its mutable fixture
type was corrected. None was converted into an expected success or fixed by
weakening a production gate.

Task-local Phase20 receipts are retained under the mi350 authoring workspace.
The sampled envelope is 112 GiB total retained task storage, >=40 GiB disk free,
>=64 GiB available RAM, two jobs, offline/locked dependencies and bounded commands.
It is not hard process-memory enforcement or transitive toolchain attestation.

These checks catch supported typing/materialization mistakes, wrong finite
scalar results, changed backing bytes/canaries, stale source/cursor joins,
missing source pages and incompatible native shapes. They do not prove absence
of races, physical-register hazards or all input-dependent faults. Checked
production scheduling, protected proof freshness, physical helper/control/memory/
matrix contracts and the shared tiled curriculum remain with their existing owners.
