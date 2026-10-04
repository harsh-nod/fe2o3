# Runtime identities and storage-reuse qualification — 2026-09-23

This report qualifies the bounded CPU debugger integration for #281 V2. Together
with the earlier M1, V1 and U1 exits, the accepted ledger is **4/18**. The three
umbrella issues remain open. V2 does not imply V3/V4, GPU capture, whole-kernel
assembly support, or production proof/artifact authority.

## Exact implementation and qualification scope

The opt-in `--runtime-observations v1` owner connects sealed simulator observations
to the existing JSONL debugger, local loopback bridge and browser. Existing wire
formats and default runtime choices remain unchanged. The implementation adds:

- Real invocation, activation, operation-attempt and parent identities; current
  and suspended frames remain distinct from arbitrary source-variable mappings.
- Allocator-owned semantic allocation, reusable storage-slot and generation
  identities. Exact-shape private/workgroup backing storage is reset before reuse.
- Independent bounded origin/frame/lifecycle retention, cursor-bound paging,
  access and memory queries, and explicit unavailable/truncated states.
- Source/SSA, storage, lifecycle and bytes/init panels bound to the exact
  connection, owner and full cursor. Control changes and disconnect clear stale
  state; a reused slot cannot silently retain an old selected allocation.
- A separate static native register-role/use grid over the unchanged existing
  14-artifact comparison. Declared roles and observed instruction uses are not
  physical-register contents, lifetimes, free ranges or GPU measurements.

The ordinary-source lab uses real Bundle V6/KIR V11 loop/helper export and the
unchanged Bundle V5/KIR V10 two-workgroup reduction. It does not repack V5 into V6
or replace source lowering with handwritten KIR. Each source run has eight
reuse-on and eight reuse-off executions, canonical/seed71 equality, independent
output/guard/init checks, 32 actual helper activations, and four strict same-build
CLI request admissions. The workgroup source exercises genuine workgroup
storage reuse; private Alloca reuse remains separately tested at a lower level.

The actual fault check in `runtime_fault_owner_tests.rs` is an unchanged 245-byte
raw-KIR fill with four invocations and a one-u32 allocation. Its real simulator
bounds fault is not an ordinary-source fault or a browser fault snapshot.
It requires exact diagnosis, current-owner/full-cursor refusal, terminal
`NoSelectedRecord`, and restoration of the previous captured checkpoint.
No terminal memory, frames or release event is invented.

## Acceptance mapping

| V2 requirement | Actual evidence |
| --- | --- |
| Snapshot values/init, source and SSA | Real loop/helper checkpoints and current/suspended-frame DOM-to-response checks; unavailable source/value cells stay unavailable |
| Repeated loops/helpers | Fresh same-depth child activations, real parent/call identities, and historical reverse/repeat restoration |
| Allocation reuse | WG0 allocation A released; WG1 allocation B uses the same slot with generation1→2 and previous A; zero bytes/false init, exact scope, old A absent |
| Reverse equality and stale rejection | Public sealed source observer, actual HTTP break/watch/reverse/repeat, and full owner/cursor/query refusal controls |
| Break/watch and faults | Real HTTP/browser breakpoint/watchpoint exercise plus the separate genuine raw-KIR CPU fault test above |
| Real browser integration | Separate actual desktop/mobile loop and workgroup runs; no routed responses, fake backend values or extra capture requests |

## Reproduction and retained runs

Build and run the public [ordinary-source acceptance](../runtime-observations-source-v1.md)
with fresh scoped caches and matching tools. The companion
[call/storage lab](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/runtime-observations-source-lab-v1.md)
and [storage protocol guide](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/runtime-storage-observations-v1.md)
describe the user workflow. Task-private HTTP/browser supervisors and their exact
input pins are retained with the receipts below; the public source runner alone
does not qualify those transport/browser layers.

All work ran over SSH on mi350. Both compiler candidates retained peer changes
through integration base `ae300712570bdd6104e2a97463c4682bcd0bfdcd`; their owned
implementation postimages match. Site base:
`a7b66c499522a1b1077f7aa47ac0a1b7c22a5ec8`. Earlier selected targets, codegen
libraries and failed outputs remain retained.

The earlier b60 common-base runtime qualification used 6,684 files,
101,349,404 bytes, SHA-256
`5dae58a3263edfd1de0ddafa92db11440263f479875e5015e19ff0c762a26e74`.
Its 733 Rust passes/5 ignored, source, HTTP and browser receipts below remain
historical qualifications. Earlier independent compiler and mirror runs each
passed 724 tests/5 ignored plus their own source/HTTP checks; current tree equality
is not a new independent mirror execution. The later 7bf peer changes were
tutorial scripts/configuration, separately checked below.

The ae300 peer regression exposed excess default-profile residency in the new
inline allocation descriptors: the unchanged schedule-budget test reported
289,341,838 bytes against a 268,435,456-byte limit. Descriptors now occupy
fallibly reserved out-of-line storage only when enabled. Preflight separately
charges their capacity; reservation precedes cache retirement and identity
commit. Reuse moves that storage without reserving again. Default execution
allocates no descriptor heap. Schedule, resident, allocation and cache caps
are unchanged. Test-only fixture filenames were also corrected for repository
policy. These compiled changes required fresh runtime qualification, not
relabelling the earlier passes.

The fresh six-crate test/build/strict-Clippy gate passed **769 tests, 5 ignored**,
including the unchanged budget regression, the new descriptor controls, the
ordinary-source observer example and canonical-V12 CLI cases. Formatting passed.
Fresh source acceptance passed eight reuse-on/eight reuse-off runs, 32 helper
activations and four same-build admissions. Both actual HTTP profiles and all
four loop/workgroup desktop/mobile browser cases then passed against the newly
selected binaries.

The fixed compiler build/source/HTTP census is 6,685 files, 101,399,063 bytes,
SHA-256 `1584e78a23292f245b5ecd19694b1dd78ef16921d480b79d8503d95fed107234`.
The full 172-case site regression and fresh actual-browser gate share the
710-file/17,162,252-byte site census,
`c0510d44cfc28b9ee1f81542b7689f30b2d8f65c6225c2d09f93804f5b25dbb2`.
Qualification/status documentation was finalized afterward. These are
working-tree qualifications, not clean-commit or hermetic closure attestations.

Earlier site checks passed 1,129 unit tests/84 files, 21 lab tests and 170 browser
regressions; unit/lab/lint/types/build passed again after the table-layout fix.
The two new 320px synthetic controls and a later complete **172-case browser
regression** passed. That full suite also validated retained evidence; it is
separate from actual backend qualification and checked zero remote issue states.
The fresh actual-browser gate separately repeats lint/types/build/evidence
validation. Historical 709-file layout/browser receipts retain their own
`1ba49e248a786960907cd0231370d78437dc60ea33b188bd504e92db2c42f6c0` census.

Earlier integrated controls passed 44 fetch-observer tests, 26 workgroup browser-owner
tests, 12 source-runner tests, 75 bridge tests, 73 tutorial-manifest tests,
48 tutorial-identity tests, six fixture-binding tests and six inherited-DCO
exception tests, plus affected formatting. The default tutorial manifest validator
passed its source-contract check while retaining its existing unqualified/pending
semantic-oracle and GEMM lesson-binding status; this is not full curriculum signoff.

## Fixed-backend qualification receipts

Paths are relative to /home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/logs/;
each row identifies its complete receipt.json. All ten runs passed, with their
individual source/input scopes preserved.

| Run directory | Bytes | SHA-256 |
| --- | ---: | --- |
| phase28-resume-r2-compiler-observed-resident-fix-build-r2 | 25121 | `da9a65fdf639abd61b3c4944f8d387c396c338d319cb34b20f82f0dcbdf69c01` |
| phase28-resume-r2-compiler-observed-resident-fix-source-r2 | 21585 | `3f8b0f6c87f0a9d182c5ec706c7f8cb2481d98cb137c0cea85b2c6d617c65705` |
| phase28-observed-source-resident-fix-r1 | 702370 | `551a826f4ce70923c1e40b56b39ffe043d7a98e1f413a7c07aa6da4c11cc9e2a` |
| phase28-resume-r2-compiler-observed-resident-fix-http-r1 | 121876 | `d93079abe96240612da124e7a11f940ff4c2d5f3e88be5669e29d3ec541c39fb` |
| phase28-resident-fix-runtime-http-loop-r1 | 209426 | `7d612e37c9da0064384bfbb3a85537178d407ab3bca6922bcc11126b13595ff3` |
| phase28-resident-fix-runtime-http-workgroup-r1 | 318105 | `4d3d1c7945374c39851191f2f6604743bbcfaa28dd1725ca688d343a5cb4d56f` |
| phase28-resume-r2-site-observed-final-browser-regressions-r1 | 26391 | `37e5d94355c1c40112661d3b8e1d5e263716f2ca750be1c368ec3f996d1806af` |
| phase28-resume-r2-site-observed-resident-fix-actual-browser-r1 | 153717 | `74de372601d276435b27e639f391d1cce1fe477e91768bbfdb79ab14d195ae66` |
| phase28-resident-fix-runtime-browser-loop-r1 | 15454 | `bc9e947bdb41e34364af99c7112b8eb74ca5efcec6e9d148e22c9dcdd7ee0748` |
| phase28-resident-fix-runtime-browser-workgroup-r1 | 15773 | `be99fa62c0c2f46756976c58ce3671582ec0582b4da8e49ef19f7d64e1e3151a` |

## Earlier retained receipt identities

Paths below are relative to /home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/logs/.
SHA-256 binds the complete retained receipt, not a screenshot or selected excerpt.

| Run directory | Bytes | SHA-256 |
| --- | ---: | --- |
| phase28-resume-r1-compiler-observed-current-main-build-r1 | 25164 | `0ee7d9bd213d6f8681c0155ba36eee23b15de0e68febc6043070cf67172d316a` |
| phase28-resume-r1-compiler-observed-current-main-source-r1 | 21585 | `3ff00a79ed47d5a778a20ff9708770009e1be9be78299369afbce3804857a8f0` |
| phase28-observed-source-current-main-r1 | 702426 | `c57f8e759c5647a9f236be5672370f19bfbb312876908aaed89f5c5fa702aea5` |
| phase28-resume-r1-compiler-observed-current-main-integration-http-r2 | 39935 | `4dc070ad2bcd12eabe8bd690812351dfa19248bedf361d38cbe56654cfcef87f` |
| phase28-current-main-runtime-http-loop-r1 | 209469 | `a977e83e0c05e85c36b0aaf9ad719f3f950fda55e8dd5c61e97e9804a46c0ac6` |
| phase28-current-main-runtime-http-workgroup-r1 | 318119 | `e3c0c4a8f061859d2065df223eadc15bcc95c6e1c1a2516f41205223c9a830aa` |
| phase28-resume-r1-site-observed-current-main-check-r1 | 27559 | `4926dd7b239e6057cb8605e1637fe479d603534dbdcd9ca59d57ff2c464d901b` |
| phase28-resume-r1-site-observed-responsive-targeted-r1 | 21328 | `0b4d0960913167d7a1352e09509cfeb01877318f65f7fbd7bedb734e85cff177` |
| phase28-resume-r1-site-observed-current-main-actual-browser-r4 | 45762 | `4a0d9b680ba1e4137c91a2526142ed320a6e1306084b3f5eb5176b52357cfd4b` |
| phase28-current-main-runtime-browser-loop-r3 | 15470 | `0ba2c1fdca7db6004eb4c8f64c62a5760846c571dc1e66a8256e03b8c0e05f9e` |
| phase28-current-main-runtime-browser-workgroup-r1 | 15788 | `28c367fe4c06f6b3a9a84ef26bf72fce1b33b276f269bc173d6e632e277ef93b` |

## Actual transport, browser, and retained failures

The fresh fixed-backend runs reproduce the scoped behavior below with separate
new identities and receipts; earlier captures remain historical.

The fresh HTTP runs passed110 requests/97 first-session CLI commands/two connections
for the loop, and143 requests/142 CLI commands/one connection for workgroup reuse.
The two actual browser runs each passed desktop and mobile as separate fresh
sessions and independently observed/reaped two actual debugger children. Their
DOM-to-wire comparisons, explicit disconnect, stale-selection clearing and
light/dark narrow-panel assertions are independent of synthetic UI fixtures.
The browser does not claim all output words completed; the separate source
runner establishes output/canary correctness. Browser screenshots do not
establish a pixel-review or performance qualification.

All failed runs remain retained. Earlier loop HTTP succeeded inside a combined
command that failed at the subsequent workgroup launch; that enclosing command
was never called a pass. A separate workgroup run then passed. Read-only port
quiescence now checks both listener ports between runs; it does not reserve
a port or prove the exact cause of the earlier startup failure.

The earlier browser R1-R8 attempts under phase28-runtime-browser-compiler-rN
failed first on an exact-label selector assumption, then on Chromium inspector
response-body loss. The selected Playwright implementation could also refetch
a missing body, so that body-observation path was removed entirely. The new
task-private observer calls native fetch exactly once, returns its original
Promise/Response to the application, and reads a bounded clone. Every body
joins bijectively to real network request/response metadata. No headers or credentials are retained; no route stubs, retries or
additional HTTP calls are used. Limits remain1MiB per
response,4096 chunks,35seconds,4MiB aggregate and100 POSTs/200 HTTP events per
project. This observation has overhead; no performance claim is made.

Current-main browser loop R1 and diagnostic R2 then failed the unchanged mobile
width assertion. Numeric diagnostics measured a376px panel with620px tables
and636px scroll width. Global table min-width620px was the cause. The production
fix resets only observed-panel table min-width/max-width, retaining local
horizontal scrolling. A new320px synthetic regression and the actual original
browser assertions pass after that fix. Site check R2 also retained a setup failure after its unit/lab/lint/types/build
passes: the evidence command omitted its explicit compiler repository argument.
The corrected evidence command passed in R3, whose full browser run then stopped
on the new test’s incorrect nested-panel selector (44 passed, one failed, one
interrupted,126 not run). The selector was corrected to the actual sibling panel;
the separate targeted desktop/mobile run passed. Final actual-browser R4 repeats
the corrected evidence/lint/type/build gates. Neither failed enclosing site
command is relabeled as successful.

The failed actual-browser R1/R2 wrappers remain:

- site-observed-current-main-actual-browser-r1: `4973f2c0849018b601dc965b9318ede67bc0f55db8a310112621a790b9cd3089`.
- site-observed-current-main-loop-diagnostic-r2: `d3e92253eb840bb946b40f85123938d4e58d5457ec4350634aa867066850be9b`.

The separate final source-policy command passed at the intermediate 7bf base:
73 tutorial-manifest, 53 identity and 13 fixture-binding tests; default source
contract validation retained its stated pending cells. Workspace dependency
policy passed for 140 members, eight layers and 487 internal declarations.
Its receipt is `phase28-resume-r1-compiler-observed-final-source-policy-r1/receipt.json`,
23,115 bytes, SHA-256 `82e1419c9ad641516ae9ecebc0d624db0d9b2d2efb3227d169b7580b7dded980`.

## Latest retained regression and setup failures

The latest-peer lifecycle group passed 19 tests and failed its unchanged resident-budget
case before the descriptor fix. The first fixed-build attempt was explicitly interrupted
to repair a remaining test-fixture path; it is not a completed build. The first fresh
source attempt refused missing empty cache-root directories before creating capture
outputs. New attempts then passed with those setup requirements corrected. None of
these enclosing failed commands is relabelled as a pass.

| Run directory | Bytes | SHA-256 |
| --- | ---: | --- |
| phase28-resume-r1-compiler-observed-latest-peer-regressions-r1 | 12707 | `1adb7899cb8e3552d6d659c85b567b1193989c03505f56356ee827d4f59154de` |
| phase28-resume-r2-compiler-observed-resident-fix-build-r1 | 14047 | `8d1062980cded21c712a8e5a0951ae5e316a8448ba64698ab77a34bd53856697` |
| phase28-resume-r2-compiler-observed-resident-fix-source-r1 | 13015 | `5c287e973c532ac206eaaa5b26c4f5609c5caa988f20319588623a966c196c0f` |

## Remaining milestone work

V3/M5 still need broader exact compiler resource/lifetime and variant projections;
the static role/use grid does not close them. V4 still needs target-bound bank
analysis and real supported hardware cells. Full curriculum/scale/owner-contract
requirements remain open.

U2 is explicitly one supported promoted region, not whole-kernel M2. Existing
materialization, fresh frontend/CPU/native flows cover much of that path. The
remaining reviewed acceptance must join those results and qualify applicable
old-analysis/proof/capture rejection at existing consumers. A diagnostic branch
that never imports a protected proof cannot claim to have invalidated one.
No new finalizer/proof route, arbitrary IR resume or whole-body requirement is
inferred here. U3 remains the checked existing-model recipe workflow.

Source authentication, protected proof/artifact/launch authority, hardware
observations, physical register contents/lifetimes, and complete kernel
correctness are not granted by these receipts.
