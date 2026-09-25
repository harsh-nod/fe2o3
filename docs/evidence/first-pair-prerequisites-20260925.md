# First-Pair Prerequisites: 2026-09-25

Status: qualified component changes, not a completed SIMT/tile pair or a
#275 M0-M6 exit. No GPU launch or protected finalization was performed.

## Source Identity

- Base: `bfa616c996fa529da67f2f6c32e7829213914e3e`.
- Tested candidate: `aaa2566f2642dfdcaab7b81e87bdbb06b8f538c2`.
- Candidate tree: `fb3416ef383a5e837f79ae73257449a9aee1e86b`.
- Candidate source fingerprint:
  `d2eb8f14871d5366571f970be59afba591fe2ca3eb91e5cdf263bd51d4bc7f4f`.
- Implementation commits: `4157ec8734`, `97e0eca9fc`, `aaa2566f26`.
- Execution host: SSH `mi300x`.
- Toolchain: `nightly-2026-04-03`, rustc
  `55e86c996809902e8bbad512cfb4d2c18be446d9`, LLVM 22.1.2.

The report's later documentation-only commit is not a new compiler test pin.
All nineteen scoped gates retained the same candidate HEAD and before/after
source fingerprint. Three read-only agent reviews reported no blocking findings
in their respective changes; the primary owned edits and execution.

## Changes

1. The private formal-memory disjointness helper now treats invocation domains
   as vacuous only when both accesses name the same singleton. The previous
   either-zero-singleton shortcut was unsafe for unequal ranges. Current
   production extractors supply equal zero-based ranges, so this is prerequisite
   hardening, not a demonstrated current production bug or native unblock.
   Frozen receipts, public APIs and current extraction domains are unchanged.
2. The existing semantic-SSA adapter accepts a masked tile load's exact
   Workgroup shared borrow at argument zero. Source and reference events,
   reborrows, move kills, type/argument checks, and escape/fork rejection remain
   enforced. This recovers a previously reviewed private integration fix; it
   does not activate tile execution or import the private scalarizer.
3. Derived V2 curriculum reports expose known variant obligations, variants
   lacking source bindings, and unregistered positive display occurrences.
   Mixed variants, identical cross-target selections, stale registered
   bindings, runtime/no-runtime reports, and legacy V1 output are tested.
   No input schema, identity, source binding, or qualification status changes.

## Validation

Rust commands used `--offline --locked`, one build job, no incremental
compilation, dev/test debug level 1, the existing shared serialization lock,
bounded timeouts, and the unchanged 12 GiB combined private target/scratch cap.
HIP and HSA runtime environment switches remained disabled.

| Gate | Result at the candidate |
| --- | --- |
| New distinct-invocation tests | 6 passed |
| Complete kernel-IR library | 833 passed, zero failed/ignored |
| Formal-memory integration | 52 passed |
| Execution borrow tests | 9 passed |
| Complete Pliron library | 1,612 passed, zero failed/ignored |
| Execution-discharge lowerer subset | 14 passed |
| Genuine provider descriptor/ABI regression | 1 passed |
| Normal backend library check | Passed |
| Ordinary row source export, CPU execution and replay | Passed |
| Identity / manifest / binding / occurrence suites | 56 / 79 / 28 / 12 passed |
| Production matrix shell harness | Passed |
| Workspace/dependency policy, hygiene, changed-file format, diff | Passed |

The focused 6 and 9 tests are included in the respective complete library
counts. The matrix harness reruns reporting tests; these are not additional
independent test counts or actual compilation of every tutorial kernel.
The provider regression uses real Rust provider descriptors, but is not a
source-to-tile execution test.

Before the production fixes, the new conflict tests produced one pass and five
failures, while the borrow suite produced seven passes and two failures. Those
red logs are retained, not relabeled.

The row gate checks 86 independent oracle cases across gfx942/gfx950 CPU
profiles and canonical/seeded scheduling requests: 688 execution/replay runs,
plus 20 simulator refusals and two stale-schedule refusals. These are CPU
observations, not compiler distribution-schedule equivalence, exhaustive
scheduling, physical GPU observations, or launch authority.

Representative commands, under the environment above:

```sh
cargo test --offline --locked -p fe2o3-kernel-ir --lib
cargo test --offline --locked -p fe2o3-kernel-ir --test formal_memory_obligations
cargo test --offline --locked -p fe2o3-pliron --lib
cargo test --offline --locked -p fe2o3-lower-mir-kernel --lib production_execution
cargo test --offline --locked -p rustc-codegen-fe2o3 --lib genuine_execution_descriptors_reject_substituted_types_and_abis
cargo test --offline --locked -p rustc-codegen-fe2o3 --test production_neutral_workgroup_reduce_driver_v1 ordinary_row_affine_source_matches_oracle_and_replay -- --ignored --exact --test-threads=1
python3 -I -B scripts/tests/tutorial_kernel_identities.py
python3 -I -B scripts/tests/tutorial_kernel_manifest.py
bash scripts/tests/kernel-compile-matrix.sh
```

The archived wrappers retain all exact commands and limits, including serialized
Rust test execution.

## Outstanding Gates

The required native LLVM positive was rerun both on the base and candidate.
Both failed at the first gfx942 LDS kernel with one formal-memory
inter-invocation conflict (exit 101). They did not reach gfx950 or native row
qualification. The failure is not waived or counted as an expected-negative
success. Access-specific analysis and an agreed receipt/consumer integration
remain necessary; the row's per-workgroup leader is not a grid singleton.

Full workspace formatting also failed on the candidate (exit 1), in two
unchanged files:

- `crates/fe2o3-pliron/src/production/semantic_ssa/partial_moves.rs`
- `crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/ordered_composition_publish_v1.rs`

Changed-file formatting passed. No whole-compiler CI success is claimed.

The existing private #285 scalarizer comprises 33 unique paths across its two
integration batches. It was located, not reimplemented or imported wholesale.
Current dependency reconciliation, authenticated source transport,
row-limited views, branded reductions/scratch epochs, actual final-graph memory
admission, generated-host preparation and direct-KFD execution remain open.
Coordination is recorded in [#271](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5839809626).

## Curriculum And Evidence

The new report was checked against the retained runtime projection pinned at
website `2ee23b1fe2a2c4e448cf40d0143009d748ceb0cf`, tree
`977e763899d2bb6a43d2f7c3ecb1bf6b5a8478a4`. This is not a new live-site test.
It retains 61 known identities, 123 variant obligations, two source-bound
variants, 121 variants without source bindings, 28 pending display bindings,
and 27 unregistered positive display occurrences. There are zero source-bound
pairs and zero qualified pairs; the exhaustive denominator remains unknown.
The website and its existing qualification pins were not changed.

Artifacts are retained under:

`mi300x:/home/harsh/work/fe2o3-issue275-paired-20260917.sp9wFtRZ/validation/`

| Artifact | SHA-256 |
| --- | --- |
| `pair-next-qualification-20260925.tar.gz` (58 retained files) | `3d0b810dbe9313c8121895764a63877decd661228012c2af32f40176a909ba02` |
| `pair-next-candidate-gates-20260925.tsv` | `7e97da4af5400524d5a0b2ba443d19aece6decacbbcc1ffe3eb1ba5c0f171463` |
| `pair-next-runtime-report-20260925.json` | `771b2fe4f4b35f06352bcf1b53fb91831fe8d8a4be90036702d8a3e15cf4f071` |
| `row-runtime-projection-20260925.json` | `176bcec0ddfc3fdd38ef46a06e781ec651538448dd01bb2c6df3c6cd844611eb` |
| `pair-next-candidate-target-llvm-20260925.log` | `6c8381682adb5f32b5e48b825fe2de7225620b1fc51c7c96ab8a279c4e6bf93d` |
| `pair-next-candidate-full-fmt-20260925.log` | `6b8346f24777e8275235cddd0fbece9ec6a51677803f3ea9ba682bd41d42676c` |

The archive includes closed green/red/outstanding logs and exit files, both gate
summaries, runner scripts, and the pinned projection/report. Build caches are
disposable; Git source identities and retained evidence are preserved.
