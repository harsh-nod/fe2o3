# Generated BF16 helpers and imported binding storage

The source-owned BF16 publisher can now be exercised end to end: select the
original Rust computation, publish a separate typed helper candidate, compile
that candidate afresh, and compare actual CPU observations against an independent
oracle. A separate genuine-source checkpoint measures one imported bindings
owner without changing its exposed ordinary compilation result.

These are compiler-developer qualifications at
[254eb55f43aabc40b627d5dca2e80725b52ac599](https://github.com/harsh-nod/fe2o3/commit/254eb55f43aabc40b627d5dca2e80725b52ac599).
That earlier campaign did not qualify a public promotion command, the normal
BF16 ranked route, physical register control, a GPU launch, or all-action memory
usage. The later [public source-authoring guide](bf16-source-authoring.md)
records separate qualification of the actual source-only CLI at
[89e06d9619ef89302e1399906b293a86f6f4d6ad](https://github.com/harsh-nod/fe2o3/commit/89e06d9619ef89302e1399906b293a86f6f4d6ad).
Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**.

## Generated source and fresh compilation

The actual 3,950-byte input is
[tiled-region-inspection-v1/src/lib.rs](../crates/rustc-codegen-fe2o3/tests/fixtures/tiled-region-inspection-v1/src/lib.rs).
Its SHA-256 is
`fcb26135ad4f931bb8dda63d631639a34dd8461e7801c22a1f6a383e0e735a3e`.
The original publisher selects the live HIR/MIR relation and original ledger;
a detached digest or edited recording cannot replace that owner.

Publication preserves the original file and creates a separate Rust file.
It moves only the selected multiply-accumulate expression into a body-local
typed helper. The original kernel still loads its fragments and stores only
returned component zero. It does not become a complete GEMM.

| Session | Source being compiled | Publication or observation |
| --- | --- | --- |
| 0 | Original direct computation | Publish Identity; observe original values |
| 1 | Fresh Identity candidate | Observe the newly imported helper |
| 2 | Original direct computation | Publish Swap01; original values stay unchanged |
| 3 | Fresh Swap01 candidate | Observe the intentionally changed helper result |

Identity returns `[values[0], values[1], values[2], values[3]]`.
Swap01 returns `[values[1], values[0], values[2], values[3]]`.
Only fresh compilation of Swap01 changes which matrix component reaches the
store. Selecting a future edit never changes the original graph being observed.

The generated Identity source is 4,381 bytes, SHA-256
`cf90e7f1e7f7031bc4845af0f80d52423d5c6e5e56c06e7e487953e30faa785d`.
Swap01 is 4,377 bytes, SHA-256
`f6a51a0d339eeae1c1aebeebb4e87bb44244a9a1f22dccc7412ae300148dfd86`.
These hashes identify source bytes, not source authority or permission to launch.

## Numerical and failure checks

The completed campaign used twelve child processes and four actual compiler
sessions. Each session checked eighteen positive CPU requests and sixteen
expected refusals: **72 positive requests and 64 refusals** in total.
The corpus uses six input patterns and output lengths 64, 13 and 0.
The numerical profile remains exact integral gfx942 BF16/F32 m16n16k16 Wave64,
not general BF16 rounding or gfx950 numerical support.

Each of the four 105,440-byte binary sidecars retains actual matrix and caller
values, input backings, output backings, initialization data, canaries and store
observations. The parent decoder independently checks dense integer matrix
results, return order and the actual output sink. Unsymmetric inputs distinguish
Swap01 from Identity. Full-wave masks remain lossless; JavaScript floating-point
numbers are not used to transport u64 counters.

The negative cases cover uninitialized input, unsupported numerical values,
execution/record limits, stopped or failed observation delivery, and wrong
launch shapes. A stopped observation does not prove cancellation or rollback.
A complete sidecar footer means CPU recording completed, not that compiler
postflight, normal lowering or a GPU launch succeeded.

All nine unchanged legacy source sessions also passed: four direct-source
sessions and five helper-source sessions, including their wrong-launch,
callback-error and callback-panic controls. Their original Rust acceptance
checks remain authoritative for this regression; the legacy parent preserves
raw JSON without claiming independent numerical replay of that JSON.

Fresh generated helpers still encounter the exact typed unsupported nominal
source-ranked BF16 consumer. The test requires that refusal. Removing it needs
the missing caller capability, tensor layout, full-wave and result-permutation
obligations, not merely a passing CPU run.

## Imported bindings storage

A separate four-session campaign compares ordinary compilation with an observed
compilation and two deliberate storage denials. It measures the original
post-import `ExtractionOnly AuthenticatedProductionBindings` owner.

| Measured quantity | Observed value |
| --- | ---: |
| Inline header | 656 bytes |
| Logical retained heap | 5,148 bytes |
| Total logical retained storage | 5,804 bytes |
| Visited items | 50 |

The ordinary and observed sessions produced the same complete **1,502 exposed
ordinary-result bytes**, including ranked IR and the reported root/check/write
facts. This is a full byte comparison of that exposed result, not only a digest
comparison and not serialization of every private compiler owner.

Zero byte and item allowances separately return the original
`Storage.Counter(ByteLimit)` and `Storage.Counter(ItemLimit)` variants.
Other failures cannot masquerade as those expected denials.
The checkpoint is not peak RSS, physical BTree allocation, a complete
128 MiB pipeline bound, recipe replay or LLVM/GPU equivalence.

## Validation and retained evidence

The backend regression passed 3,701 outer tests plus six nested test executions;
219 tests were ignored. The optimized driver selection passed 450 tests,
with 216 ignored. All 37 newly added controls passed in both modes.

The first generated-source campaign failed in its parent validator because it
confused the compiler canonical identity with SHA-256 of serialized canonical
bytes. The correction preserves both domains and adds four discriminating
controls; all sixteen parent schema controls passed. The failed attempt remains
failed. An entirely fresh campaign produced the successful evidence below.

| Completed gate | Root receipt SHA-256 |
| --- | --- |
| Generated source and CPU campaign | `24643504025999890d73bee3eb243711da00c8e9223b53b2f91fcd4096b14834` |
| Original bindings checkpoint | `234a52197bd2fb7b19dac541f96dd979fd44a60d27c3343fa7a1165c88c5a8b9` |
| Legacy direct-source regression | `f063ec0c1f041d5c9f9a346fccaa445bdd6bb48ccd2189c09aedb5374ed13097` |
| Legacy helper-source regression | `691c6a28e593df088690228a4a15c265df37db6d401ba688136ce23b9521fea7` |
| Integrated optimized regression | `e5f2d618a88e05204ba13687a512bc82170c4a2f58e28041b677ccce5a18ba2b` |

The genuine campaigns used source census
`6bf98861560fdb3ead79b8ca347432bcaf16030c3057ba2c08e01ebf73fbb787`
(9,701 files, 136,331,479 bytes). Publication preserved the concurrent
documentation-only commit `8ecfb2a5`; the subsequent optimized regression used
`89b3f94268bf2d6ba26c1836a5ce9f5e47c2d74003f09454e52682ad97a1a118`.
The latter is not a claim that the genuine campaigns were rerun after that
documentation change.

The source adapters and inert controls are committed. The campaign orchestration
and machine-specific preparation are retained qualification tooling, not a
supported public workflow. The later public guide covers direct extractor
inspection and source publication with a current rustc invocation; it does not
turn this historical parent into a public replay command. A clean-checkout
end-to-end setup, normal BF16 production continuation, debugger integration and
hardware qualification remain separate work.
