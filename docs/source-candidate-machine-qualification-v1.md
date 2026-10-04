# Source candidate/register-edit machine observation — r3 qualification

Status: actual-source frontend, simulator and diagnostic LLVM export **passed**.
Native worker/code-object observations are recorded separately below; GPU execution
is **not established** by either receipt. This private test-only slice is not production continuation or
completion of #282 U2.

This note was checked independently against the retained report, gate receipt,
eight subprocess logs, current harness implementation, and exact saved source/
LLVM bytes. No compiler or test was rerun while preparing the note.

## Evidence identity

Task root: `/home/harmenon/fe2o3-authoring-280-282.FEW3gj`.

- Report: `phase11-source-machine-r3/observation.json`, 49,606 bytes,
  SHA-256 `b024053ddf9263675ef43faea69b6f137eb9ad82b6ec19a68214e03827529e31`.
- Gate: `logs/phase11-source-machine-r3.json`,
  SHA-256 `9213a25a711718df0d71b280b600c81338b0332bfa940dfe0f4d9882aa1be0df`.
  Exit 0, no signal/stop reason, 48,911 ms; test stdout records one passed ignored
  ladder selected explicitly (23.00 s test runtime).
- Repository HEAD: `b8f7abf8c661fec5b7f16049777393defac0ba8f`.
  The exact qualified worktree is additionally bound by the unchanged before/
  after input census:
  `8788c22879b07b23e434aa1f97f6dea6429466020082514177825f430331e724`,
  3,766 files, 73,619,030 bytes, zero deletion markers. This is a worktree receipt,
  not a claim that HEAD alone contains every qualified change. Census covers
  Rust, Cargo and crate README inputs; new JavaScript runner work is excluded.

Exact ignored test selector:

```text
production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::machine::ladder::actual_source_candidate_machine_ladder
```

The receipt ran `cargo test --offline --locked -p rustc-codegen-fe2o3 --lib`
with that selector and `-- --exact --ignored --nocapture`, through the primary's
pinned resource-guarded runner. A rerun requires a new output directory/run label;
the retained r3 evidence must not be overwritten.

## What ran

Seven actual rustc callbacks were independently counted: one ordinary Rust
baseline; default candidate, register-edited candidate and fresh repeated edited
candidate; three exact negative callbacks. The dedicated source-publication
subprocess is an eighth successful subprocess, **not** an eighth rustc callback.
Each retained child stdout contains one expected report and one passing test
completion.

The original expression `b ^ ((a ^ b) & mask)` is joined from typed HIR parameter/
initializer identities and original byte offsets to actual semantic/KIR
correspondence. Its genuine baseline owner is **KIR V8**, function `choose_bits`,
block 2, operations 0/1/2, results 13/14/15; no V8 bytes are relabeled as V17.

The same-session checked materialization emits a separate candidate. A retained-
source, no-replace edit then changes only its explicit register plan:

- Default: scratch v4, output v5, inputs v0/v1/v2.
- Edited: scratch v32, output v33, inputs v34/v35/v36.

The XOR/AND/XOR instruction sequence and source parameter roles remain unchanged.
Each candidate is reparsed by a fresh actual frontend; the resulting genuine
semantic **V32** / KIR **V17** owner is checked against the expected program and
source roles. The simulator and existing gfx942 LLVM emitter borrow that **same
live V17 owner**. Prior source admission, diagnostic serialization and previous
verification evidence are not reused as authority.

Three positive fresh callbacks each ran 15 whole-kernel scenarios twice:
five scalar triples × output lengths 0, 1 and 65 × deterministic repeated execution,
for **90 positive simulator runs** total. All compare the full output backing,
initialization and canaries against the independent host expression
`(a & mask) | (b & !mask)`; input requests remain unchanged. Each positive records
33,940 simulator steps, 86,510 prepaid host-payload bytes, and zero incomplete
race/conflict assessments. These are bounded observations, not a functional or
race-freedom proof. Additional execution in the wrong-output negative is bounded
separately and is not included in the 90 positive-run count.

The edited and repeated-edited observations are exactly equal, including their
current owner and LLVM identities. The default/edited source, semantic, KIR and
LLVM identities differ. The actual fresh input SSA values also differ (2/8/7
versus 9/2/10), while source parameter ordinals remain 1/2/3: the edit is freshly
joined, not rebound by assuming unchanged SSA numbers.

## Exact source and LLVM payloads

Sources are under the repository's ignored directory
`target/source-bitselect-candidate-roundtrip/phase11-source-machine-r3/positive/`.
Hashes below were recomputed from the retained files, not merely copied from JSON.

| Payload | Bytes | SHA-256 |
| --- | ---: | --- |
| original.rs | 1,259 | `5072283b41b4c8dc2e364c13e95971d8fdfdb8d8993c6a95d3440665d720c33b` |
| candidate.rs | 1,481 | `a3835695aa1b7685646b69a1bdb0fd4e2842ca6f7d0bad8aeef44506e56064d4` |
| edited.rs | 1,486 | `8c8f82fe6b05a49b2195a18bde0811213e7f0a3677e5e0968083ab887daa3e29` |
| wrong-output.rs | 1,479 | `44692346ba1492084f53a1f527700d53eceb3ee949533e08515e7a99d7d1dadc` |
| stale-edited.rs, after deliberate mutation | 1,536 | `0609df111dd56eddaefebebc7af0e8cd3fc3459863ce6fca7b4455bb0e0032c6` |
| default.ll | 2,015 | `edd769a91f7b4be6248e908c0c9de3d81c940b65929303f9eb3e9feaee4f8eff` |
| edited.ll | 2,025 | `42d29b0c7cbf2b944c7b0a5a1c387262dbcac292f14e24003a9226270c48a758` |
| repeat.ll | 2,025 | `42d29b0c7cbf2b944c7b0a5a1c387262dbcac292f14e24003a9226270c48a758` |

LLVM files are under `phase11-source-machine-r3/positive/`, outside the repository.
The stale-source fixture was originally published with the same bytes/hash as
edited.rs; its later mismatch is intentional and is not an inconsistent receipt.

Current default KIR V17 digest:
`ec41628202a9abee84511b062785f19a73f5daa6926a740dcd2d5aff7044526b`.
Edited/repeated KIR V17 digest:
`1442c87790864a745c7225407e2518fe338cce0fefb0a77abedc9acde1a8f2db`.

The exported edited LLVM contains the three fixed-register inline instructions,
their early-clobber output and scratch clobber, and stores the actual returned
value through the checked whole-kernel output path. Its reported VGPR binding
high-water 37 is **not** a qualified final hardware descriptor/resource count.

## Exact refusal coverage

All three negatives reached one actual rustc callback and the named intended
boundary, rather than treating an unrelated earlier compiler failure as success.

- Wrong expected register plan:
  `source-candidate fresh program/role binding differs`.
- Correct program but deliberately storing the wrong value:
  `source-candidate machine oracle output, initialization, canaries or identity mismatch`.
- Candidate changed after retained-descriptor capture:
  `source-candidate retained bytes differ from parsed compiler input`.

No diagnostic LLVM is published for these three refusals.

## Bounds and retained failure history

The source census independently totals ten regular files (five sources plus
loaders), 7,593 bytes, under a ten-file / 1,310,720-byte envelope. Original and
default candidate remain unchanged; edits use separate no-replace destinations.
The output analysis directory remains empty.

- Each simulator callback: at most 30 runs / 4,000,000 cumulative steps /
  2 MiB charged host payload; existing per-execution limits also remain active.
- Edit scan: 1,461,799 work units and 397,850 payload bytes, below 32 MiB work /
  1 MiB payload envelopes. Each retained publication reports three source-I/O
  calls, with the existing bounded I/O accounting unchanged.
- LLVM: existing 16 MiB generation cap; at most 64 KiB published diagnostic text.
- Gate: 15-minute wall limit, 1 MiB per stdout/stderr stream, 40 GiB free-disk
  floor, 64 GiB available-RAM floor, combined 20 GiB cache/callback cap. R3 ended
  with 114,774,155,264 free disk bytes and 9,282,020,887 charged combined bytes.
  Failed r1/r2 callback directories remain included in the retained resource
  accounting; no failure receipts were removed.

The preceding failures remain at `logs/phase11-source-machine-r1.*` and
`logs/phase11-source-machine-r2.*`, both exit 101:

1. **r1 — wrong parent working directory.** Baseline/default progressed, but
   the parent test ran from the backend crate while retained source I/O requires
   repository-relative paths. Publication correctly refused the nonexistent
   relative parent. The fix runs publication in a dedicated repository-root
   subprocess; it does not mutate global cwd or relax the source-I/O API.
2. **r2 — ambiguous fixture edit.** The broad `*slot = selected;` match also
   occurred in an inactive-cfg fixture. The editor correctly refused ambiguity.
   The fix selects the unique post-macro store context; direct inspection of the
   retained edited source finds two broad matches but exactly one qualified
   anchor. Ambiguity refusal remains intact.

R3 passed after these harness corrections and a clean-rebuilt primary. Its exact
frozen source census, not an earlier failed census, is the qualification input.

## Limits of the source-only r3 receipt

The report explicitly retains `native_inspection_pending=true`,
`ranked_checks=false`, `production_resume=false`, `functional_proof=false`,
`hardware_observed=false`, and no artifact/launch authority. Fresh source-map
capture is also unavailable in this slice.

The next separate native observation must consume the exact edited LLVM hash
above through the existing bounded C++ worker/encoding path (observer arguments
`three used <edited.ll>`). O0/O3 inspection, final object/resource metadata,
native linking/HSACO and GPU execution are not implied by an LLVM text export.
No production source-admission surface, public continuation, policy, schema or
milestone-completion claim is introduced by this qualification.


## Subsequent edited-source native observation

The separate native join passed at phase11-source-machine-native-r1/receipt.json
(90,296 bytes, SHA-256
e7cfa85903530a53cd94c64a2fb621b3d29534bc6b2daf9e93685d3a56227dc1).
Its exact native observation is 10,387 bytes, SHA-256
7c0fa162e82171ce045c4775756747c458be2c9c8782ee7879503a07420f92fd.
The unchanged retained native worker ran once (49 ms), with arguments
three used <absolute path to r3 positive/edited.ll>, exit zero,
no signal/timeout and empty stderr. Eight pure adapter controls passed separately;
they are not counted as source, native or hardware executions.

The adapter rejoined all seven actual rustc callback reports and invocation files,
the current qualified Rust/Cargo/README census, ten source/loader files, all three
positive LLVM outputs, exact negative refusals, and the intentionally stale-source
append. It remeasured the native build's recorded inputs, selected link libraries,
binaries, worker claim and stage logs before and after. These are consistency joins,
not authenticated process ancestry or a complete dynamic runtime attestation.

Both O0 and O3 final linked code-object inspections found the contiguous authored
XOR/AND/XOR instruction sequence with exact registers:

| Instruction | Register operands | Encoding |
| --- | --- | --- |
| XOR | v32, v34, v35 | 2247402a |
| AND | v32, v32, v36 | 20494026 |
| XOR | v33, v35, v32 | 2341422a |

The worker decoded the instructions and implicit EXEC reads. Four boundary
register sites were retained for each case; those sites do not establish physical
value histories or register allocation/lifetime correctness. The whole kernel
contains 124 instructions at O0 and 20 at O3: stability of the authored region
does not imply whole-kernel order or byte stability.

| Case | HSACO bytes | Encoded VGPR capacity | Architected VGPR boundary |
| --- | ---: | ---: | ---: |
| O0 | 6,280 | 56 | 40 |
| O3 | 5,384 | 40 | 40 |

The required explicit footprint high-water is 37. These are decoded descriptor
capacity observations, not metadata usage, occupancy measurements or lifetime
proofs. Post-link checks report gfx942, code-object version 6, exports
choose_bits / choose_bits.kd, no unresolved symbols, wave64 and
required workgroup [64,1,1].

This additional observation does not mutate the original source receipt's
native_inspection_pending=true. Final HSACO bytes are not retained by
this adapter: it retains digest/decoded observations, not a final artifact with
launch authority. The adapter remains a task-local qualification tool, with a
120-second native limit, 64 KiB native stdout, 1 MiB retained stream and 2 MiB
aggregate retained-output bounds. The 20 GiB combined-cache, 40 GiB free-disk and
64 GiB available-RAM guards are unchanged.

No GPU execution, public production continuation, functional proof, protected
admission or allocation/lifetime proof follows. #282 U2 remains open.
