# Source-authoring and source-value evidence — 2026-09-22

This dated record supplements the [bounded source-promotion contract](../source-bitselect-promotion-v1.md).
It distinguishes completed historical observations from final-candidate gates.
It is not a claim of GPU execution, protected compiler admission, publication,
or broad completion of #280/#281/#282.

## Candidate identity

Canonical base: `431fa35b6795e5170ae74858da8b2416002a8717`.
The SourceContext-refactored r6 build used 6,534 files / 99,331,543 bytes,
census `ec76b4f21bb9fc6f9ff4ed4a217f2ad1e323063a36695e5713cc220068cd21a5`.
Its normal consumer/backend build and test-harness build passed.

Mirror base: `e9d98e62254b095b29da50e1904a83ab1a6d9c79`.
The independently built mirror has 6,527 files / 99,267,144 bytes,
census `e774249edc6c4c42215095b4ec8a61148d1c7f8c176d2827196bba1156c198ec`.
Its normal consumer/backend and test-harness build also passed. These different
source censuses and bases must not be collapsed into one identity.

The final source cleanup only groups nine borrowed arguments in SourceContext.
The earlier canonical source census was
`1572a464d3b0065ae6a7ef49f74fa0ed3d2e34e9901cbe2b06023a7a295e4920`
(6,534 files / 99,331,107 bytes); the earlier mirror census was
`f7a4fbc34c6944d13b1b37017f4e53e2f30db2690939f34d30d94c87a0039c58`
(6,527 files / 99,266,708 bytes). Those earlier runs remain retained.
The final compiler results below use ec76b4f2... / e774249e... respectively.

## Completed final regression and ladders

| Lane | Canonical | Independently built mirror |
| --- | --- | --- |
| Full backend regression | 1,710 passed; 109 ignored | 1,709 passed; 109 ignored |
| Baseline source roundtrip | 90 simulations; three successful fresh variants; three exact refusals | Same observed counts |
| Public live-prefix ladder | 90 whole-kernel simulations; 17 controls / 16 exact refusals; 41 source leaves | Same observed counts |
| Public generated-negative ladder | 30 simulations; four exact resource/boundary refusals | Same observed counts |
| Public debug join | 60 simulations; two captures; three exact stale catalog/capture identity refusals | Same observed counts |

Both full regressions also passed the normal dependency-consumer smoke, all 12
source-value Node controls, changed-Rust rustfmt and whitespace checks. Existing
ignored tests were not counted as executed.

The prefix controls include one eight-binding publication-only success. They
are direct public-API calls, not 17 extra external-consumer processes.
Default/edit/repeat each enter a fresh compiler callback; the oracle includes
the preserved ordinary OR prefix and ordinary XOR suffix.
The debug identity refusals do not establish production proof invalidation.

Receipt files are retained on mi350 under
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/logs/`;
a label below denotes `phase18-LABEL/receipt.json`. These are operator-retained
files, not published artifact downloads. SHA-256 identifies bytes, not authority.

| Receipt label | SHA-256 |
| --- | --- |
| `authoring-build-r6` | `1a0efcfbc6b8ebaa0b02b841d3db2e39f5c949a95c2146c8aa5e20b06ae420cc` |
| `mirror-authoring-build-r2` | `7850a92f72a20f1764466bf41f28afb7d6064d1f8661f793d22f8bfc82aa85c8` |
| `compiler-regression-r2` | `94a5b7dd6ab41f60f65cf5f254f8b3a723d0f8b310c08f5cfcb3a266ebb606de` |
| `public-prefix-ladder-r5` | `f369c3282c67c0610cc2405a0e88ac11f80a3710a9ba4201381cdb751f7aa71e` |
| `public-negative-ladder-r3` | `be35f08069517264057600ac952549b0851467952ab6d3f93eee643eb70652bb` |
| `public-debug-join-r3` | `9810667e4bea639f00ff586af67047957f47d0e14021531cb445ef72e6029316` |
| `public-baseline-ladder-r3` | `b4050f3d2461d3d70cc57f794774d57e9ae76646eee2ad4fe18577fbf7c8c368` |
| `mirror-compiler-regression-r2` | `412219c9d639fda0ab58f2298fc099d5969fbe6e61893f60b5a9f4f34dd83ba4` |
| `mirror-public-prefix-ladder-r2` | `867dcbe33f7f3dcd72b9801d73c73eebdedf0dbbf5fc213650899a5ac98aa0a0` |
| `mirror-public-negative-ladder-r2` | `fcaa68f67026f2663f6e99313ecabe75baec888a9184edb39fadc646d015df4c` |
| `mirror-public-debug-join-r2` | `64f2692d7855fdde74a6e224e9433a1ad0efc511d111d81f8d5577e612362e05` |
| `mirror-public-baseline-ladder-r2` | `71647538c15a8b1f9b513171314028532e34fe8afbeeadb73e559c73a3c73ae8` |
| `compiler-lint-comparison-r1` | `d3f018a5c070a32c20101a7f1a48bf782bf075b6746af62e9ffb2b2da8d29527` |

## Native observations and strict lint

The final strict join `prefix-native-report-join-r2` passed four low/high O0/O3
cases from `public-prefix-ladder-r5` and the ec76b4f2... source census.
It checked 101 selected files / 810,739,619 stable bytes, including the actual
consumer, harness and backend library; two positive and 21 negative typed-chain
controls remain checked. Join receipt SHA-256:
`e0e2362661e6a87eaa2765f9434dee41a4aa8ed38f0cc7f7cb0c4c7973599be8`.

| Final native input / receipt | Bytes or SHA-256 |
| --- | --- |
| Default LLVM | 2,072 bytes; `970d7f8644f30b4e1cd5927b1829627c93af1bf81b8430b40997becb7927d93a` |
| Edited and repeated LLVM | 2,082 bytes each; `0ff412f82d32b6df0dd58d8b5003dbe641529a9b8945ecbd16031558f92ecbac` |
| Low O0/O3 receipt | `8ef33e50bc5fa2dfb292c5d0cce52bb3609ee454d513ba62e97dcc3f4ea7aea3` |
| High O0/O3 receipt | `26a38925e71b3431326fd428ff52e74c5d99e28a27a2cb638a8a08129d4e6c45` |

The earlier `prefix-native-report-join-r1` also passed, against the earlier
1572a464... canonical census (99 selected files / 566,386,417 bytes). Its receipt
`b5c230d7d28ba79f3587804f5d58ce61454136bbbcfb0e049a673e8b87f718b4`
remains historical; it is not substituted for the final run.

The new private low-plan checker preserves the exact OR-before-assembly,
XOR-after-assembly, result-to-store chain. High-plan checks reuse the retained
checker. Their selected instruction/operand/descriptor reports do not retain a
complete HSACO, qualify protected finalizer policy, prove register lifetimes,
establish hardware execution, or qualify the mirror's native output. The
task-private checker reuses a retained worker checkout through absolute build
paths; it is not a portable clean-checkout product-native pipeline.

Strict Clippy remains failed: both baseline and current report 31 errors.
Thirty match by code, message and primary source text; the other existing
dead_code diagnostic narrows from unused hir_id plus range to unused range.
There are no added diagnostics, but this is not a passing strict-lint gate.
Current receipt `compiler-clippy-r2`:
`012fa980fea24a41eb1af1310f7592934d52a4523ceeb2ff6938ed649dfbe3ec`; baseline `baseline-clippy-r1`:
`0e88c1e2a818ddd347cbe557d467ae84b0dc298c31c8509b922e98cec8cd6dc1`.

## Historical source-value capture

The [site recording provenance](https://github.com/harsh-nod/fe2o3-kernels/blob/main/examples/source-variable-resource-v2/provenance.json)
retains the earlier canonical 1572a464... census, not the later refactor.
The successful excerpt has 27 pairs / 60,542 bytes / three checkpoints.
Each retains six pages / 12 variables: two captured parameters and ten
unrepresented variables, beside independent SSA and unchanged early memory.
Four deliberate refusals remain in the full interaction. The browser still
labels this imported recording caller-supplied/unverified.

Capture receipt SHA-256:
`7e4e60ae26ab640749ffb7b417822e3151ca44cf0e1b66f3fd269984a6e74b20`;
source-export receipt:
`6d9b82265a79e7ada0c54cb968e4365ace60f01b436714e592d05d9ae390eaeb`.
See the [site qualification record](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/source-values-qualification-20260922.md)
for separately measured import and browser checks.

## Publication and retained limits

These dated documents and cross-links were added after the tested code
snapshots above. The 15 changed Rust/JavaScript leaves are byte-identical across
the independently qualified compiler checkouts; their divergent bases are
preserved. Commit identities, final policy checks and remote-main verification
are recorded in the issue handoff, not inferred from a local command exit.

Both dependency policies passed (140 members, eight layers, 487 dependencies;
unchanged pinned Pliron revision). The task uses at most 80 GiB retained root
storage, at least 40 GiB free disk and 64 GiB available RAM, two Cargo jobs,
locked/offline builds, incremental disabled, serialized qualification and
bounded process-group/stream supervision. These are sampled task guards, not
allocator/RSS bounds, complete descendant-quiescence proof or transitive build
attestation.

Earlier failed loader, prefix-attribution/negative-fixture, browser-layout,
metrics-parser and missing-historical-object attempts remain retained. No
failed result is relabeled as passing and no policy or proof check is waived.
The [contract review](../assembly-authoring-contract-review-20260922.md) retains
the original milestone exits and dependencies; this batch closes none of the
remaining broad milestones.
