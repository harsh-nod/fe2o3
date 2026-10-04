# Ordered-program region-origin qualification — 2026-09-23

Fresh ordinary Rust-source exports passed with the explicit
`--diagnostic-ordered-origin-v1` option. The additive report preserves a real
compiler-owned **whole-region** origin beside unchanged raw diagnostic KIR V17.
This is a bounded origin-handoff slice, not closure of V3, instruction-level
macro ancestry, hardware debugging or physical-register lifetime proof.

See the [tutorial](../ordered-program-origin-v1.md) for public commands.

## Capture and independently checked bindings

The retained capture ran on mi350 from a dirty source tree based at
`59604e9cc9252c27dd452152265c2e8939745450`. That base commit alone is not the
tested source identity. The before/after census was unchanged:
6,702 files, 101,549,362 bytes,
SHA-256 `65550afe74ebedd9e7b7670ac0bd204b568dc2450ccb3bb4a1dea643175e94db`.

The owner built fresh tools in `target-milestones-phase28-origin-proof-r1`.
The source gate used the normal exporter/extractor, matching backend DSO,
pinned nightly-2026-04-03 toolchain libraries and unchanged official
`inspect_diagnostic_ordered_program_v17` example. It did not reconstruct KIR
or compiler bindings, import old native capsules, run a GPU or execute kernels.

A separate read-only audit rehashed all 75 retained selected input/output pins
(772,508,414 cumulative bytes), checked their recorded filesystem metadata,
remeasured all 20 command streams and compared their hashes with the receipt.
It independently recomputed domain-separated V17 canonical identities from
the four raw KIR files and checked exact report/inspector source-ID joins.
All matched. This audit is a content/identity check, not source authentication
or proof that every transitive compiler input was captured.

## Actual source results

All three distinct inputs use the same `amdgpu_ordered_program!` init/repeat
macro. “One” is not a non-macro control. “Repeat” uses the identical fifteen-copy
source and manifest path with a separate fresh extraction target.

| Capture | Add copies | Declared instructions | Macro depth | Work used | Origin bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| one | 1 | 2 | 1 | 134,809 | 3,035 |
| two | 2 | 3 | 1 | 134,809 | 3,115 |
| fifteen | 15 | 16 | 1 | 134,819 | 4,161 |
| repeat | 15 | 16 | 1 | 134,819 | 4,161 |

All use gfx942:xnack-, wave64, scratch32/output33/inputs34,35,36 and descriptor8
followed by N copies of descriptor201. Each raw KIR file is 1,014 bytes.
The reported expansion span is byte9789..10074, line285:9 through291:68;
call sites are line12:18 through17:6, byte351..561 for one/two and351..562
for fifteen/repeat. Those are compiler-recorded spans, not inferred step origins.

Actual raw rustc MIR block0, semantic block and KIR raw-block observations differ:

| Capture | Raw rustc block | Semantic block | KIR roster coordinate | KIR raw block |
| --- | ---: | ---: | --- | ---: |
| one | 0 | 0 | [0,0,0] | 0 |
| two | 0 | 5 | [0,0,0] | 5 |
| fifteen / repeat | 0 | 2 | [0,0,0] | 2 |

Equal roster coordinates do not identify equal variants. Canonical KIR,
semantic MIR, source preflight, complete rustc MIR-body and block identities
differ across the three repetition counts. Root-function, monomorphization,
root/contract inventory and declared contract identities remain equal; they
are not generic body digests.

All ten commands passed: a legacy one-copy export/inspection without the new
flag, then four new exports and four independent inspections. Every command
exited0 with no signal or recorded failure reason. Legacy-one and opt-in one
produced byte-identical raw KIR and inspection output. Fifteen and repeat
produced byte-identical raw KIR, complete origin JSON and inspection output.

| Capture | Domain-separated canonical SHA-256 | Origin-file SHA-256 |
| --- | --- | --- |
| one | b25ca769210fea7002cbed1011c4597a3e52c726b081fc1580af6887c70495cb | ea02e063945404c28615f38049e7656ed5a5b0c9072ca58261742ea80359ee00 |
| two | bed336afd5e4a70c7e12eb221a306483c0005619861d6139c95800750e818e03 | 266f5f3826d95772d330cd48c24fd7cdf03bf483b03cb71e2b305148009251e3 |
| fifteen / repeat | 62ee53e33c561e0c8fe48b74ed9479db82bff25dba7a56ad8318a742c251a551 | 0c7ebae478a0d2aaa9c3897414decfd17c58c9e1ed9c2df202727189eb160b8e |

## Retained evidence identities

Paths below are task-retained receipts, not files shipped in this repository.
They are relative to the preserved qualification task root.

| Receipt | Bytes | SHA-256 |
| --- | ---: | --- |
| logs/phase28-resume-r2-compiler-origin-proof-build-r1/receipt.json | 23,360 | f72a2f701c6f54addaf866d030f2aca05bac5e61740df5271e58a300d986343c |
| logs/phase28-resume-r2-compiler-origin-proof-regressions-lease-r1/receipt.json | 39,805 | 872e65c34686c1621e16bf293425081cece75150f471414048f3ec7c517d440a |
| logs/phase28-resume-r2-compiler-ordered-origin-source-r1/receipt.json | 37,817 | 00e2359d435861f29b165ba9df5cac126232db0df24d976b687bc452ed8af25f |
| logs/phase28-ordered-origin-source-r1/receipt.json | 89,900 | 2ea648278eed68a6093c60376bad3ea7d0045e390db370c5f61de452b37c35e0 |
| logs/phase28-resume-r2-compiler-origin-proof-final-format-r1/receipt.json | 21,999 | 9a24b07fb8a43145b385506931cda404620dacffba8e6458f61a822c61e54530 |

Selected fresh artifact pins:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| fe2o3-export-sim | 765,904 | 432335d2b86ef6f879ff67110b8c42203918023830eca4fd44c4d5a894ee83aa |
| fe2o3-rustc-extract | 213,184 | 950d888eec31bc50f82c9f6a7db81c04db70dfafc057789ab925853bb1eb8d75 |
| librustc_codegen_fe2o3.so | 244,973,640 | c061daab0159dc3c93971bfd3af0cac28193f88c2b3affc41fe5a3a7eb663572 |
| inspect_diagnostic_ordered_program_v17 | 5,638,712 | 3c908879278e15674f5713b14bd034aa042a9c16ffa88273d31d259934c877a0 |

The shared regression batch passed 1,726 backend tests (113 existing ignored),
11 exporter tests, 24 extractor tests and 143 verifier tests (13 existing ignored).
The build/qualification lane also passed 13 script controls and 20 origin-parser
controls. These are suite totals, not counts of newly added origin tests; ignored
tests receive no qualification credit. Final format/syntax/diff checks passed.
The separate protected-proof tests in that shared batch are not evidence that
origin JSON grants proof authority.

## Bounds, corrections and nonclaims

The report caps are16KiB,16 instructions,256 macro parents and1,048,576
source-reobservation work units. Observed work is below that limit. The source
runner caps each command stream at1MiB, selected input count at512,
individual selected file at512MiB and cumulative selected bytes at2GiB.
It uses five separate fresh extraction caches and retains failures.

The executed source-r2 runner corrects source-r1's overly strict input link-count
assumption: actual Cargo artifacts have two hard links. It accepts regular
positive-link-count selected inputs while recording/rechecking exact metadata;
new owned outputs still require one link. Both revisions' original bytes remain
retained. The current prepared sources are all macro inputs, not a fabricated
plain-versus-macro matrix.

The outer receipt confirms unchanged sampled source census, successful process
exit and drained streams. It explicitly does not prove descendant quiescence or
transitive build attestation; sampled resource guards are not hard whole-process
RSS accounting. The source runner additionally requires external supervision
and a complete build-input census.

Fine-step origins and full macro expansion frames remain unavailable: the
retained program is flattened and every descriptor has whole-region association.
Compiler-policy, source-map, edit-epoch, schedule, native-artifact, physical-value
and physical-lifetime fields absent from this diagnostic are explicitly
unavailable. No LLVM/native compilation, GPU execution, performance result,
source/compiler-execution authentication, resume/launch authority, or V3
completion is claimed. No freshly captured origin is joined to historical native
evidence merely because labels, repeat counts or instruction bytes match.
