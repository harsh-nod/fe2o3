# A2 Concrete Composition Integration Handoff

Review-only source integration. No compiler, solver, CPU test or GPU execution.

## Locations

- Private integration copy: `/run/shm/fe2o3-a2-composition-integration-20261001-iPwBWDtR/src`.
- Base: `6c36c2ab11bb57cfee92d51157784a7050241508`.
- Qualified source: `fef0f92cb6385c57d1b77c2eff41db967490beb1` in the unchanged, clean `/run/shm/fe2o3-a2-concrete-journal-composition-20261001-qualified`.
- Root owns application to main, full merged workflow checks, signing and both pushes.

Exactly the candidate's 24 paths are changed or newly present in the private
copy. Nineteen remain byte-identical to the qualified candidate. No staging or
commit was performed. Root integration worktree and evidence were not edited.

## Five Integration Differences From Candidate

1. `.github/workflows/runtime-component-source-guards.yml`: added the three new
   concrete composition suites to current main. Preserved the operation-codec
   suite and current cadence wildcard. SHA-256:
   `479a0ffbbf0cf8476fa8371c5fa6609c8f97485ec875aae8002f74614af8c515`.
2. `check-producer-journal-composition.py`: only the existing wrapper-guard
   dependency pin changes from `90ae07c093d4274c25af8269a9aae7e073393e0ead9190ed059a12507f4f9430`
   to current main's `f93f0f5b6c3c503fcd0c0e98b665f5ffc6ca98a91fd1e7581bf0cb566425565b`.
   The wrapper guard itself is unchanged. New concrete guard SHA-256:
   `abcf788a2e4d9c4817f126a1868a78e293bd2016acbc56c9bd14fdcbcce03e0b`.
3. `producer-journal-composition-mutations-v1.py`: only `GUARD_SHA` changes to
   the preceding new concrete guard digest. Mutation bodies, case roster and
   boundary labels are unchanged. New builder SHA-256:
   `69af5e51fcb0973243e4efe5212beda9824675f7a1167a836dd1893f1e05c7e8`.
4. `docs/runtime-producer-input-composition.md`: qualification cross-reference
   updated, with no inherited-count or milestone claim. SHA-256:
   `e349d1d9c6fdc35171624361c59adcca9588458617b31f7d89ef6cb5bae5e5f2`.
5. `docs/runtime-producer-journal-composition.md`: explicit composed 109+3
   qualification, original 112-stage rejection retained, limits and remaining
   boundaries stated. Public evidence remains described as unpublished until
   root publication. SHA-256:
   `54a546a7c3438415ae213810fa2bc3ba517414778a4206ffb638613a3f1ee661`.

Current wrapper guard differs from the candidate parent's wrapper guard only
in its queued-query `PRIOR_SHA`; current operation-codec inventory changes are
preserved. The leaf and conditional guards need no integration change.

## Exact Closure Checks

The 6-input leaf, 8-input conditional and 44-input concrete closures all match
qualified source bytes exactly. The inherited 3-input fold, 36-input query and
38-input wrapper closures also match current main exactly. File sets were
checked for duplicates and compared by complete content, not only names.

SHA-256 of each sorted compact JSON path-to-file-SHA map:

| Closure | Inputs | Map SHA-256 |
| --- | ---: | --- |
| Leaf | 6 | `6fe78d3c998abf590e14e938dcb89267f6641f74860978eb0e71753614931c04` |
| Conditional | 8 | `5bbe62ede5335ff1b72cb8204d2bb19f5ec09ca7ee07fb5c7aec9359d9c4bb78` |
| Concrete | 44 | `619ae0f4efa6756174c4b7e9d65617b7bc2efaef32d4f356ba1821895a8c3f23` |
| Fold | 3 | `b874f685b88d1083fe68c7cafbc2e95f8ee006968d65eb071ef1b544b77d4ee8` |
| Query | 36 | `82cffca19a9c39d477925eae9fc45b46a8b1b41babf2c26ba176c0fc5669bd90` |
| Wrapper | 38 | `50375826124f8b0bcb30b8fa1c9aa7ccf055f13c874ea56587e7d57cf53367aa` |

All 328 runtime source files and all 308 model source files materialized from
current main match it exactly. No native Rust or proof logic was edited during
integration. The source guard reports 44 closure files and 664 bound sources.
Its `expected_verified: null` and `qualified: false` remain intentional:
source-control success does not itself assert solver qualification.

## Source-Only Test Results

Every command used `python3 -I -B` in the private integration copy. All passed:

| Suite under `crates/fe2o3-runtime-model/verus` | Groups |
| --- | ---: |
| `test-producer-input-validate.py` | 5 |
| `test-producer-input-fold.py` | 4 |
| `test-producer-input-composition.py` | 13 |
| `test-producer-input-composition-qualification.py` | 5 |
| `test-producer-input-diagnostics.py` | 14 |
| `test-producer-journal-observers.py` | 8 |
| `test-producer-journal-diagnostics.py` | 17 |
| `test-producer-journal-composition.py` | 10 |
| `test-producer-journal-composition-mutations.py` | 16 |
| `test-producer-journal-composition-diagnostics.py` | 17 |

Total: 109 groups. `git diff --check` also passed. This is not a claim that the
entire merged workflow ran; root should run its full signed workflow after
application. Original qualified source remains clean. Closing read of root
worktree retained HEAD `6c36c2ab...`; its only observed status entry was root's
new untracked publication directory, which this lane did not touch.

## Qualification Wording

The accepted result composes 109 strictly read-back stages from the unchanged
RAM-rejected original with exactly three fresh closing stages. It qualifies 89
unique prefix negatives, with zero new negative executions in the completion.
There are three positive brackets each at 42/0, 64/0 and 214/0. The original
112-stage owner remains rejected. Eight forwarding faults are actual
result-equality failures; eager-status is ghost-trace-only. Live/credit
freshness, Arc/mutex/shared-state and machine execution remain outside scope.
This is component qualification, not A2 completion or HIP/HSA parity.
