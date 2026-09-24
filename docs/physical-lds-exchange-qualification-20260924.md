# LDS exchange source qualification — 2026-09-24

This record distinguishes completed root-run source/CPU gates, public wrapper/CLI
gates and separate static native work. Hashes identify retained observations;
they are not compiler signatures, exported source custody or runtime authority.

## Completed actual-source gates

The pre-ranked diagnostic ladder ran 38 isolated sessions: nineteen real Rust
variants, each observed through the owned-source path and the public diagnostic
driver. Both positive layouts produced byte-identical public diagnostics.
It completed 64 CPU positive cases, 12 exact CPU refusals, two observed barrier
cases and 34 exact source refusals.

- Observation: 143,142 bytes,
  f27f7a09778a5b55e0cb44b1bf913085b9c7428681cd6189c5b0d7bc984dd243.
- Completed command receipt: 22,316 bytes,
  d311b73aa2eb8283fe0212299103ac40febd8c4a2037f0294e30fc025bdc30df.

The normal checked-source ladder separately ran 57 sessions over the same nineteen
variants: owned observation, normal LLVM and normal inert handoff. It completed
64 CPU positives, 12 CPU refusals, 51 exact source refusals, 142 actual-owner ABI
mutation controls and eight resource-denial controls. Public LLVM and handoff
were compared with the outputs retained from the genuine checked owner.

- Observation: 219,162 bytes,
  d2baf19b2687508230e27dffd98ce85d4fd0d23d27627817fec7c95c5d931760.
- Completed command receipt: 20,861 bytes,
  4813c0c3e138f4b51c28513700c0a18df3208928b3fd9a839ae19142caa23016.

The specialized CPU capture ladder used two newly authenticated source sessions.
It completed four captures and eight capture refusals; the two generic-debug
controls refused with zero records. Each positive captured 128 invocations,
two logical waves, three allocations, 128 global pending-to-ready transitions,
128 LDS pending-to-ready transitions, 128 barrier arrivals and one release.
Reverse navigation was observation-only. Same-ledger denial history and
source replay before/after capture were retained.

- Observation: 11,818 bytes,
  19236639cf8e72c2cb15cbfba58ee52ade8cd613277ee507759fa1064598d76b.
- Completed command receipt: 20,608 bytes,
  be9fef50f52416d62d8e0129136ebffcc412a1d57f84e848b16491d935c09f25.

These are historical root-run results, not tests executed by the author of the
new public wrappers. The retained fixture source digest was
cafa481d9e6a7a780c27921f276476bbded31b03ed40067c48b87204d028f870.
No clean publication commit is inferred from that source observation.

## Boundaries corrected during qualification

The initial source fixture accidentally retained a second default root; the
feature exclusion was fixed without weakening single-root admission. Negative
controls were corrected to exercise their intended source boundary: a foreign
constant slice avoids an unrelated panic path; a noinline foreign helper avoids
a separate unsupported intrinsic. Removed wait/barrier rows honestly hit the
earlier block/operation count check. Exact diagnostic oracles retain these
boundaries rather than accepting arbitrary failure.

Ranked safety projection fixes preserved the executable graph: exact SSA byte
address/index correspondence, conditional 128-element output prefix under the
retained 512-byte requirement, and local_x = global_id under exactly one
128-invocation workgroup. Existing analyzers were not changed to ignore failed
bounds or workgroup reports. The peer-index equality was checked over all
128 local indices; opaque loaded data was not converted into address-derived
values.

## Completed unchanged-source static native gate

The separate root-run R2 matrix compiled the unchanged normal-source LLVM through
the existing ordinary native worker and decoded the result: two register layouts
at O0 and O3, four cases. It checked exactly 32 instructions / 168 entry bytes,
no added executable setup/tail, all seventeen metadata arguments (four explicit
plus thirteen hidden), the 512-byte LDS reservation and no scratch allocation.
There were sixteen exact input-pin refusals and 280 native mutation refusals.

O0/O3 HSACO bytes were identical within each layout:

- One: 5,592 bytes,
  678e9b1e8c1d67d954f6052579aa931e0e95389454d30285912678131979fa2f.
- Registers: 5,648 bytes,
  7b0450316d5201e298ee0f0abfd721b8a460b8eb97eca4356083a309ff25a208.

Static descriptor VGPR capacities were 24 and 32 respectively; both SGPR
capacities were 32. These are decoded allocation fields, NOT physical samples.

- Matrix report: 250,657 bytes,
  45311b9538f5293c39e2ed2f0fcff3c2bfa49b9e6b4df11a706546a4c3d34c48.
- Completed command receipt: 294,839 bytes,
  92cb4af1f6322061cb563e72f8460621e7ce4b017eb38931adfa963cdbe66f0a.

The initial R1 strict C++ build failed on nested main-macro handling before
qualification. R2 retained that failure and used an exact one-function-name
change in its private reusable fixture wrapper; no native matcher, worker,
decoder, source LLVM or reservation repair was substituted.

This is static host-side native compilation/decoding, not execution of the GPU
kernel, runtime ABI discharge, protected artifact admission or proof that
hardware barriers published memory. Source authority was not reconstructed
from retained bytes.

## Completed public source and debugger surfaces

The public source/checked wrapper plan passed all six separately completed
shards and a fresh read-only aggregate gate:38 commands,57 extraction stages,
51 exact refusals and6 successful extraction stages. Both source layouts passed
diagnostic export and normal LLVM/inert-handoff export. Each fresh canonical,
descriptor and handoff relation was checked against its own output identity;
only LLVM bytes were compared with historical observations. No fresh identity
was normalized into an old one.

- Aggregate report:6,880 bytes,
  ff49f53944d9e546cd738b0f38848ff2fc815e45b9ab460720ef6f35a7240aca.
- Completed aggregate gate:198,803 bytes,
  b45982a59e72ae557b5f3c02265f437ce470b7a9709df0a7a15fbd2f77274624.
- Pure wrapper controls:30 passed. The six shard outputs used19,942,784,260 bytes
  in total, within the separately bounded plan; prior failed outputs were retained.

The explicit public V22 debugger CLI/index gate independently passed36 pure
controls and28 actual direct children:6 sessions spanning both register layouts
and output extents129/13/0,96 transactional refusals,13 bootstrap refusals,
8 byte-exact V20/V21 replays and one cumulative-budget denial. The latter retained
383 responses from512 submitted requests before denial. Each positive recording
contained three allocations, two logical waves,128 barrier arrivals/one phase0
release, final byte/init/canary checks and four pending/ready/reverse-pending
query relations. The authored publication epoch1 is distinct from barrier phase0.

- CLI report:41,158 bytes,
  e1042fbc3e0b97afafa0f0aef734556ecdea7bea1baaa5427cf914a9ba40efb8.
- Completed CLI gate:119,040 bytes,
  4b98d3542ad5758f70ee24c12cd9d1ee230e323302cc21cd11b4eb7f0b92a5c6.

The first CLI qualification incorrectly expected LDS memory before declaration.
The corrected test first requires the actual transactional refusal, then checks
zero-storage/uninitialized bytes at the same-site after-declaration checkpoint.
The producer's lifetime semantics were not weakened. Root independently rehashed
all101 retained CLI files (22,637,448 bytes). These CLI recordings derive from
the earlier actual sourceR7 exports, not the fresh public-wrapper outputs.

Public wrapper/CLI child exit is not a whole-process-family cleanup proof.
Counts from overlapping source, wrapper, CLI and native gates must not be summed
as disjoint coverage. The separately authored website reader and its publication
have their own evidence and compiler pin.

## Not established by these results

No physical GPU registers, source-variable mapping, kernel execution, host binding,
protected finalization or launch admission was observed in these LDS gates. Runtime
allocation/kernarg conditions remain unresolved. CPU short-output controls do not
relax the formal512-byte output requirement. This finite two-wave LDS slice does
not close the wider memory/synchronization or hardware debugger milestones.

## Merged-tree regression

The September 24 R6 gate passed on functional commit
`bf86faa9e531bb2932c77f2a851649bd5cef00b3`, preserving all 56 paths from the
concurrent conditional-formula/native-issuer update. The compiler and mirror
had identical trees. The completed gate is 31,749 bytes, SHA-256
`a959fc2298bf717d0822021d7d75f3c90a47d9d4e019e61850ecc09c2b87c368`.

The source census stayed unchanged: 7,659 files, 112,355,765 bytes, SHA-256
`f4bbc0f66736b6a2c67f1d574710ed4f85ec0d27ec84e2536ea8900b2cc736ba`.
It reported 10,823 passing Rust test executions over 239 result groups,
zero failures and 238 explicit ignored tests. These are overlapping executions,
not 10,823 distinct tests, and ignored actual-source/native gates are not implied
to have run. Separate Node/Python checks and the documented focused, actual-source,
CPU-recording and static-native gates retain their own scope. Existing compiler
warnings remain; this is not a workspace-wide warning-free claim.

The earlier R5 gate remains failed: two compiler-execution-client socket tests
exceeded Linux's Unix-domain pathname limit under the long task TMPDIR. R6 gave
only that package a short, private /tmp alias to the same task-owned scratch
directory; it did not alter the socket protocol, tests, deadlines or assertions.
All five client library tests passed, alongside its remaining targets.

Publication policy is checked against the already-published main base, not
rewritten historical commits. The earlier wide-range DCO attempt reported seven
inherited commits without this repository's exact trailer; those historical
commits were preserved unchanged. The incoming commits are signed off.
Stored unified patches preserve their required context-prefix whitespace via
three exact .gitattributes paths; their bytes and qualified reconstructed debugger
sources are unchanged. No source-code whitespace rule is disabled.
