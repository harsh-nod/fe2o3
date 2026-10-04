# Concrete conditional authoring: source and CPU qualification — 2026-09-23

The experimental `const_if` spelling passed normal-source admission and CPU
acceptance independently in both compiler forks. Each run used fresh normal
tools and exported complete registered Rust kernels: four exports, four
inspections, 120 simulations and eight exact frontend refusals (136 stages).
This is finite source/CPU evidence, not LLVM/native, protected production,
hardware, performance, or broad milestone completion.

The [authoring guide](../ordered-program-select-source-v1.md) records syntax,
all eleven exact source fixtures, command templates and representation limits.
The [earlier repeat-native report](authoring-repeat-native-20260923.md) remains
separate: its different kernel/source inputs do not qualify selection.

## Implemented boundary

A concrete bool constant selects one of two complete flat programs. Both arms
are checked independently by the existing packer, including an inactive arm,
and each requires 1..16 instructions with valid roles and initialization.
The final program still uses one existing terminal134 marker, one u8 count,
four u64 descriptor words, three u32 data operands and five literal u8 roles.
The words are source descriptors, not native machine encodings. Runtime input
expressions evaluate once and in order. No GPU branch, executable schema,
source-owner replacement, generic-dependent predicate or compiler bypass is added.

The new macro arm introduces no caller-scope const items or local bindings.
Five direct helper projections avoid the demonstrated identifier collisions.
Each eager call checks at most 32 descriptors, at most 160 across the five
syntactic calls; this is not a bound on predicate CTFE, compiler RSS or I/O.
Old flat/repeat arms and the actual terminal declaration are unchanged.

Selected functional leaves are byte-identical in both forks:

| Leaf | Bytes | SHA-256 |
| --- | --- | --- |
| crates/fe2o3-device/src/ordered_program.rs | 12660 | `5a34a40e827d024fed121c88b3cdab6913fbc5e1593b4d86ea1d3573f61633bc` |
| crates/fe2o3-device/src/ordered_program_select_v1.rs | 5218 | `cfdc3313f3d8bd14c7fb90e0aa0cbac73f5c993549509303ffbf888d5473f379` |
| crates/fe2o3-device/tests/ordered_program/select_v1.rs | 10882 | `c7c450c5535954575e076cb66e7dbf26ee020267a0e4e15c120750e562318660` |
| scripts/ordered-program-select-source-smoke.mjs | 38213 | `e60c97e954755bb0582fe970cc330e384f84cddcd9a49c8b20ba271b454ab588` |
| scripts/tests/ordered-program-select-source-smoke.test.mjs | 40293 | `487f06481ef519d63e991f406e861bca4e1c2b9ad2a3b85bb592b216ddfb6114` |

## Provider closure and tested source snapshots

The device source roster grew from 28 to 29 regular files (411206 raw bytes).
The unchanged canonical Cargo manifest is 398 bytes, SHA-256
`da7bbcd2f3dac75b968f45137c920d94658b5600ff52bf9d843d494bcdd8afb7`;
the existing vendor-normalized manifest is 1922 bytes, SHA-256
`a5505445b6b63f1e46b7fca58aa25450de19f3cec1c8d44ce444e64d441a22b4`.
There is no build.rs or new auto-discovered integration-test target.

Using the unchanged domain and sorted raw path/content length framing, the
current canonical closure is
`ffbfcf5fceccdad6f01b4b761502c94845829a26e8ead7496c992122d2ef1dbc`
and vendor closure is
`b543e896235b425e328c53c158c207882b20e5512fcde455066c0573e8d13991`.
Exactly the two existing current arrays, canonical known answer and roster
count were refreshed. No stale compatibility entry, semantic-transcript
known answer, hashing algorithm or admission rule changed. Independent review
recomputed both closures. Historical fixture-README prefixes differ between
forks and were preserved; only their identical provenance appendix was added.

| Fork | Parent HEAD | Files | Bytes | Full functional snapshot SHA-256 |
| --- | --- | --- | --- | --- |
| compiler | `c4e994c6883e70aafc87101cf4239d4048cc8428` | 6616 | 100499823 | `67f7504df1702eb9b88a5ef7659149f0cdd2159985a13d1b17e7721f44e35853` |
| mirror | `7fc820e1022f24a0eff2167783d24b1302052d07` | 6609 | 100438698 | `924a4b9f2c13c52e69c516cf60f35a3d1184a01d9e0b45c4bc7e89ae833c4335` |

These are actual source-capture censuses before this report and final
Markdown-only additions. The canonical tool build preceded adding the two
new JavaScript leaves; its own retained census records that difference.
All Rust/provider bytes were unchanged between that build and source capture.
The mirror tool build includes the final functional tree. Reports are not
retroactively relabeled as build inputs, and the full census is not a source
signature or protected compiler-closure attestation.

## Actual acceptance and independent oracle

The selected target is gfx942:xnack-, wave64, with required/maximum workgroup
64x1x1. Literal true and named const true select MOV/ADD; named const false
and its identical-source repeat select MOV/ADD/ADD. Declared registers are
scratch32/out33/inputs34,35,36. High-water 37 is a declaration, not final
allocation, occupancy or physical-register lifetime proof.

For each export, five scalar triples, lengths 0/1/65 and two replays produce
30 CPU cases. The independent BigInt oracle computes `(a + n*b) & 0xffffffffn`,
where n is 1 or 2 from the selected source alternative, not from interpreting
emitted descriptors. Each fork checks 2640 output words, 11520 total backing
bytes, complete initialization/padding and all eight canary bytes per run.
All 60 request buffers start a5/uninitialized; immutable scalar arguments and
empty-output canaries are checked. These are selected CPU runs, not a universal
algorithm or race-freedom proof.

| Fork | Sum of stage elapsed ms | Retained file pins | Unique retained pin bytes |
| --- | --- | --- | --- |
| compiler | 170396 | 423 | 433212685 |
| mirror | 167086 | 423 | 433218581 |

Times are observations on mi350, not approved performance/latency budgets.
Independent review checked original selected pin ledgers, raw streams,
output buffers, initialization and refusal correspondence. Exact-source
repeat reproduces raw KIR, export identities, inspector result and simulation
summaries. Literal/named true compute the same formula but have different
observed identities here; no general source-spelling identity rule is inferred.
The retained inventory remains a root/contract census, not a body digest.
File SHA-256 and domain-separated semantic/canonical identities stay distinct:

| Fork/variant | Steps | Semantic identity | Canonical identity | Raw KIR SHA-256 |
| --- | --- | --- | --- | --- |
| compiler/literal-true | 2 | `c050168675d7d79ce9a6fd61cf798cd8027bd544fb1406baf5ca24d1d23be88e` | `983a05ec680e638625fe1804b328b15c51cc7c6fa890e3d34e98fb95380090e7` | `3eef699935c203c6018fd8fee1dc5672f601a0ff8fdd862cac87eb063b921978` |
| compiler/const-true | 2 | `5d3a082c45b5a29c51b4fe921ec8b6f8917d214044ec8f8b320bd84b2f32b544` | `ea2e26be3133c0d3bef1b093e3e8e255315a4415bc4ac1450528bc40a04851ba` | `179fbdedad4a2fedbdfc563685c3f2f4471129f4f7309b08d45a1c80e853d511` |
| compiler/const-false | 3 | `9889b495b815500725e99bbe2797627e3491eb21f59af13893bf8b0b24b68d3a` | `bd12337236dce298cf59d83be55ed23cbfcad7df9380708d47a3b0fcffa40253` | `92d7182c483bfc58e191e1ec1b854b1d22fd464d4d7d82b135b4c5c8d98b995d` |
| compiler/repeat | 3 | `9889b495b815500725e99bbe2797627e3491eb21f59af13893bf8b0b24b68d3a` | `bd12337236dce298cf59d83be55ed23cbfcad7df9380708d47a3b0fcffa40253` | `92d7182c483bfc58e191e1ec1b854b1d22fd464d4d7d82b135b4c5c8d98b995d` |
| mirror/literal-true | 2 | `428059c1523ff4527a71ae68e8e26766607001fccd66ad6bc4319b4828c22021` | `4cd3726f4ce05302dafa54455524a05cb96973828fc7109729fe38892ae9d145` | `9e0d2c4aa2dc61f5fe8e5c085674c61662c3826f3c9be5b78df961b291d64a5b` |
| mirror/const-true | 2 | `49c18cc36c10a179002b1851c05489705591e2e1d4e0a618e3553cf50e5f4f74` | `7d44f248c01dee7f2667014507cd65b4117a08edfefe82b1829d25f85343c6b9` | `059bff1236c0f35590c881fa6574a2c38cda54234c6c8333c016a40ea61734f2` |
| mirror/const-false | 3 | `6f397cb17c48fb580eea110e0db326b185b578c51257782f98a7bc0a383f762d` | `dd9a6b7416a77f09242ea0c0c8f5dac8d3cfcb4138893f9534593479425aa2de` | `995b91da84b666e3526e4b1e15af05d8bfcc57102f6c2e59bc7ccb6be262bce7` |
| mirror/repeat | 3 | `6f397cb17c48fb580eea110e0db326b185b578c51257782f98a7bc0a383f762d` | `dd9a6b7416a77f09242ea0c0c8f5dac8d3cfcb4138893f9534593479425aa2de` | `995b91da84b666e3526e4b1e15af05d8bfcc57102f6c2e59bc7ccb6be262bce7` |

All exported raw diagnostic KIR V17 files are 1014 bytes. This route is not
Bundle V6, source-authenticated execution, a physical-register trace, or
per-instruction debugging; the existing whole-region logical operation remains.

## Exact frontend refusals

Each negative is a full registered Rust kernel consumed by the actual normal
exporter, with its own fresh package/target. Cargo exited 101 and the exporter 1
for the intended cause; no rejected case produced KIR. Every typed Rust
primary retains the exact target, manifest/source, code/message and primary
span presence. The alias is the actual first existing source-owner diagnostic,
not a host type-check failure.

| Case | Primary count | Cause |
| --- | --- | --- |
| dynamic | 1 | E0435: non-constant value in a constant |
| non-bool | 5 | E0308: mismatched types |
| nested | 1 | Exact unsupported macro syntax |
| missing-else | 1 | Exact unsupported macro syntax |
| unknown-opcode | 5 | Exact binary opcode/arity refusal |
| inactive-undefined | 5 | E0080: undefined source read |
| inactive-seventeen | 5 | E0080: requires 1..16 steps |
| register-alias | 0 Rust primaries | Existing owner: distinct physical roles v0..v63 |

Both real captures matched this complete roster. Timeouts, missing tools,
loader/dependency failures, warnings, unrelated errors and arbitrary cascades
are not accepted as negatives. Generic-dependent predicates remain a separate
Rust compile-fail control, not an invented generic-kernel source route.

## Regression results and retained failures

Each fresh tool-build gate passed pinned rustfmt, 194 existing JavaScript
controls and 164 authoring Rust tests, then built the normal compiler DSO,
exporter/extractor, author, inspector, simulator, debugger and lowerer example.
Building the example is not an observed selection LLVM lowering.
Each final regression passed 245 JavaScript controls (including 28 new selection
groups), 1711 backend tests with 110 existing ignored tests, 21 ordered-program
plus 2 ordered-region tests in both debug/release, the no_std library check,
9 runnable/no-run doctests and 22 compile-fail doctests, 151 Python controls and
shell syntax. Ignored backend tests are not counted as passes. Existing
backend/target-feature and duplicate fixture-target warnings remain retained;
no strict Clippy pass is claimed.

An earlier isolated host language probe demonstrated const-item capture in
the unintegrated R1 selection expansion. Ordinary control compiled; the four
introduced predicate item names and one runtime-operand collision refused.
R2 removed those names, and actual device regression groups cover the former
predicate/data identifiers and once/in-order evaluation. The retained host
diagnostic preview used stripped host snippets, not full registered kernels;
it is separate from both later real source captures.

The first combined canonical regression stopped on an extra closing
parenthesis in the new JavaScript test file: 217 checks passed, one test file
failed to parse, and backend/device/Python checks had not started. Exactly
one `)` was removed in both candidates; frozen private drafts and that failed
gate remain retained. The complete R2 regression then passed. No refusal
predicate, fixture or production implementation was relaxed to fix the test.

## Retained evidence and limits

All paths below are relative to the retained task root on mi350:
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr`.
They are custody references, not public download links or authority tokens.
The inner source receipts pin 423 selected files each and are independently
pinned below; a receipt cannot authenticate its own contents.

| Receipt | Bytes | SHA-256 |
| --- | --- | --- |
| logs/phase25-compiler-device-select-r1/receipt.json | 21565 | `473e18280396d9a3e39b5556e5cc5aa8929914899f246006e1500665267ed7e3` |
| logs/phase25-compiler-hygiene-probe-r1/receipt.json | 28393 | `acca74552eee991c5f120ba0161ca415d75249eee39da7679bb511c9df47f891` |
| logs/phase25-compiler-macro-diagnostic-preview-r1/receipt.json | 57274 | `555a43a48bb7f01cea55b47ceced578d2f26b7c7da70910a5139aabb4838879e` |
| logs/phase25-compiler-tool-build-r1/receipt.json | 26508 | `751b95213c89970908054f2cd9735fb7a5d1bfdc0bd6b6d9244fce21b35c3e9a` |
| logs/phase25-compiler-select-regression-r1/receipt.json | 21806 | `6e290fb853ef40490041ce896b63b5f80ba9510717fee5104a828ddaa049e6d5` |
| logs/phase25-compiler-select-regression-r2/receipt.json | 40248 | `b973bac4954198177b6a46ab937dfb2d1c6eaf010d4f793c0ed0021edf5a67eb` |
| logs/phase25-compiler-select-source-r1/receipt.json | 36227 | `3524a293b31d352ebb5d2c1431efa09e5c05586692610e2767c7a43c622da725` |
| logs/phase25-mirror-tool-build-r1/receipt.json | 26687 | `b5528a388aedce72e12d949f535620f07848048f03052ffc9adb43685634ccdf` |
| logs/phase25-mirror-select-regression-r1/receipt.json | 40530 | `35e6a70446a8d873412f05f2d87f6c0440f3f3f0bcb459d6e254490d644d549a` |
| logs/phase25-mirror-select-source-r1/receipt.json | 36131 | `bba691f45bda90a559aae963c9d836b9781dd41e3e4161fb69c2b2ce2180b7c6` |
| logs/phase25-compiler-select-source-actual-r1/receipt.json | 363279 | `83dcb2ae5af29eedf97a9eec9b81d669210194a6873dc7064d6f39171167c59d` |
| logs/phase25-mirror-select-source-actual-r1/receipt.json | 361817 | `ad1c1402840cb154cfd47a500f7f76984a7017b373dcb3b48ac4e39f68bd79f6` |

The pinned Rust toolchain is nightly-2026-04-03 and Node is 22.22.3.
Cargo used locked/offline inputs, jobs 2 and incremental 0. Fresh target roots
are `target-milestones-phase25-compiler-r1` and
`target-milestones-phase25-mirror-r1`; no old backend DSO was adopted merely
because a path existed. Selected executable/loader pins and before/after
identities are in the outer receipts. No complete transitive runtime closure
or escaped-descendant quiescence is claimed.

The outer scope caps the retained task root at 160 GiB with 40 GiB free disk
and 64 GiB available RAM reserves, per-command 20-minute deadline, 8 GiB output
and 8 MiB per stream. Highest sampled root size across these gates was
153269433446 bytes. Source capture additionally caps 136 sequential stages,
19-minute cooperative time, 300s/export, 60s/query, 64KiB source/KIR, 1MiB JSON/
streams, 512 pins, 512MiB/file and 2GiB unique selected bytes. These sampled/
cooperative guards are not OS quotas, continuous RSS limits or hard real-time
proofs. Compiler scratch and all failed attempts remain retained separately.

The paired Markdown lab changes no curriculum pin, maturity label, runtime,
owner schema, production policy or hardware claim. Existing M1/V1/U1 remain
3/18 accepted broad exits; this completes a bounded constant-selection
capability, not M2/U2/U3 or another broad milestone. Runtime control, whole-body
ABI/resource ownership, memory/hazard semantics, selection-specific LLVM/native
qualification and applicable production/hardware gates remain separate work.
